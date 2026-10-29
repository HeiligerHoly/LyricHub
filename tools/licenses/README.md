# Third-party notices

`THIRD_PARTY_NOTICES.md` in the repository root is generated from `about.hbs` with
[cargo-about](https://github.com/EmbarkStudios/cargo-about) 0.9:

```bash
cargo install cargo-about --locked --features cli
cd src-tauri
cargo about generate --locked -c ../tools/licenses/about.toml ../tools/licenses/about.hbs -o ../THIRD_PARTY_NOTICES.md
```

cargo-about fills in the Rust crates of the Windows build. The sections above them (IPADIC,
Inter, the npm packages in the frontend bundle, NSIS) are plain text in `about.hbs`; update
them by hand when one of those packages changes its version or license. The installer ships the
file next to `lyrichub.exe` (`bundle.resources` in `src-tauri/tauri.conf.json`).

The align plugin has its own notices, `plugins/align/THIRD_PARTY_NOTICES.md`, from
`align.hbs`; the sections on ONNX Runtime and the model are plain text there:

```bash
cd plugins/align
cargo about generate --locked -c ../../tools/licenses/about.toml ../../tools/licenses/align.hbs -o THIRD_PARTY_NOTICES.md
```

`onnxruntime-ThirdPartyNotices.txt` is ONNX Runtime's own file from
https://github.com/microsoft/onnxruntime/blob/v1.31.0/ThirdPartyNotices.txt (the version the
`ort` crate links); fetch it again when `ort` moves to another ONNX Runtime. The plugin ZIP
(`plugins/align/package.ps1`) carries both files and `LICENSE.txt`.
