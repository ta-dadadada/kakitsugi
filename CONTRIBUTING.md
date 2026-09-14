# Contributing to Kakitsugi

コントリビューションを歓迎します。大きな仕様変更、データ形式の変更、安全境界に関わる提案は、実装前に Issue で目的と影響範囲を相談してください。不具合修正は、可能な限り再現手順または回帰テストを添えてください。

## 開発環境

Rust 1.92.0 を使用します。`rust-toolchain.toml` により、rustup が必要なツールチェーン、rustfmt、Clippy を選択します。

```sh
git clone https://github.com/ta-dadadada/kakitsugi.git
cd kakitsugi
cargo build --locked
```

## 品質チェック

Pull Request を作成する前に次を実行してください。

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo package --locked --allow-dirty
sh -n install.sh scripts/test-install.sh scripts/test-packslip.sh
sh scripts/test-install.sh
python3 scripts/test-distribution.py
mise exec github:jdx/packslip@1.2.0 -- sh scripts/test-packslip.sh
```

最後のコマンドは Packslip 1.2.0 で一時的な署名付きマニフェストを生成・検証します。mise を使わない場合は、同じバージョンの `packslip` を `PATH` に置いて `sh scripts/test-packslip.sh` を実行してください。

Web UI を変更した場合は、通常幅と 760px 以下の幅で、キーボード操作、200% 相当の拡大、空状態、API エラーからの再試行も確認してください。UI 資産は `assets/` からバイナリへ埋め込まれるため、変更後は Rust バイナリを再ビルドしてください。

## Pull Request

- 1つの変更目的に絞り、利用者から見える変更と理由を説明してください。
- 公開 API、CLI、設定、保存形式、利用手順を変更した場合は、README と関連する `specs/` を同時に更新してください。
- 生成物、SQLite データベース、秘密情報、個人情報をコミットしないでください。
- セキュリティ上の問題は公開 Issue や Pull Request にせず、[SECURITY.md](SECURITY.md) の窓口を使用してください。

コントリビューションは、このリポジトリの [MIT License](LICENSE) の下で提供されます。

リポジトリ管理者向けの GitHub 設定は [メンテナー向け公開チェックリスト](docs/maintainer-guide.md) を参照してください。
