## 変更内容

<!-- 何を、なぜ変更したかを記載してください。 -->

## 確認内容

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked -- -D warnings`
- [ ] `cargo test --all-targets --locked`
- [ ] インストーラーを変更した場合は `sh -n install.sh scripts/test-install.sh && sh scripts/test-install.sh`
- [ ] 公開 API、CLI、設定、利用手順を変更した場合は文書を更新した
- [ ] Web UI を変更した場合は狭幅、キーボード操作、エラー回復を確認した

## 関連 Issue

<!-- 例: Closes #123 -->
