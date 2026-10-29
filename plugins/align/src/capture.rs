//! Process loopback (PLAN-plugins.md 5.1): the sound of the player's process tree only, never a
//! game's, as 48 kHz 16-bit stereo mixed down to mono. A plain Windows interface, the one OBS and
//! Discord use; no hooks, no injection. Every packet carries its QueryPerformanceCounter time in
//! 100 ns, the unit of LyricHub's playback anchors.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{BLOB, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Variant::VT_BLOB;
use windows_core::{HRESULT, IUnknown, Interface, Ref, implement};

pub const RATE: u32 = 48_000;

pub struct Capture {
    pub pid: u32,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    /// Starts capturing `pid` and its children. `sink` gets each packet (QPC time in 100 ns and
    /// mono samples); `failed` gets the reason if the capture cannot start or breaks.
    pub fn start(pid: u32, mut sink: impl FnMut(i64, &[f32]) + Send + 'static, failed: impl FnOnce(String) + Send + 'static) -> Capture {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            // SAFETY: COM for this thread only; every interface is dropped before uninitialising.
            unsafe {
                if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                    return failed("COM".into());
                }
                let result = run(pid, &flag, &mut sink);
                CoUninitialize();
                if let Err(e) = result {
                    failed(e);
                }
            }
        });
        Capture { pid, stop, thread: Some(thread) }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Done(mpsc::Sender<()>);

impl IActivateAudioInterfaceCompletionHandler_Impl for Done_Impl {
    fn ActivateCompleted(&self, _op: Ref<IActivateAudioInterfaceAsyncOperation>) -> windows_core::Result<()> {
        let _ = self.0.send(());
        Ok(())
    }
}

unsafe fn run(pid: u32, stop: &AtomicBool, sink: &mut impl FnMut(i64, &[f32])) -> Result<(), String> {
    fn e(what: &'static str) -> impl Fn(windows_core::Error) -> String {
        move |err| format!("{what}: {err}")
    }
    // SAFETY: the activation parameters outlive the call; the buffer pointers from GetBuffer
    // are read only until ReleaseBuffer.
    unsafe {
        let params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: pid,
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };
        // The blob points at `params` on this stack: the PROPVARIANT must never be cleared, or
        // PropVariantClear frees that pointer and corrupts the heap.
        let mut pv = std::mem::ManuallyDrop::new(PROPVARIANT::default());
        {
            let inner = &mut *pv.Anonymous.Anonymous;
            inner.vt = VT_BLOB;
            inner.Anonymous.blob = BLOB { cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32, pBlobData: &params as *const _ as *mut u8 };
        }
        let (tx, rx) = mpsc::channel();
        let handler: IActivateAudioInterfaceCompletionHandler = Done(tx).into();
        let op = ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&*pv), &handler).map_err(e("activate"))?;
        rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "activation timed out".to_string())?;
        let mut hr = HRESULT(0);
        let mut unk: Option<IUnknown> = None;
        op.GetActivateResult(&mut hr, &mut unk).map_err(e("activate"))?;
        hr.ok().map_err(e("activate"))?;
        let client: IAudioClient = unk.ok_or("no audio client")?.cast().map_err(e("audio client"))?;
        let fmt = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: 2,
            nSamplesPerSec: RATE,
            nAvgBytesPerSec: RATE * 4,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, &fmt, None).map_err(e("initialize"))?;
        let capture: IAudioCaptureClient = client.GetService().map_err(e("capture client"))?;
        client.Start().map_err(e("start"))?;
        let mut mono = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(10));
            while capture.GetNextPacketSize().map_err(e("packet"))? > 0 {
                let (mut data, mut n, mut flags, mut qpc) = (std::ptr::null_mut(), 0u32, 0u32, 0u64);
                capture.GetBuffer(&mut data, &mut n, &mut flags, None, Some(&mut qpc)).map_err(e("buffer"))?;
                mono.clear();
                if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data.is_null() {
                    mono.resize(n as usize, 0.0);
                } else {
                    let s = std::slice::from_raw_parts(data as *const i16, n as usize * 2);
                    mono.extend(s.as_chunks::<2>().0.iter().map(|p| (f32::from(p[0]) + f32::from(p[1])) / 65536.0));
                }
                capture.ReleaseBuffer(n).map_err(e("release"))?;
                sink(qpc as i64, &mono);
            }
        }
        let _ = client.Stop();
        Ok(())
    }
}
