# メンテナー向け公開チェックリスト

この文書は、GitHub リポジトリを一般公開するときにリポジトリ設定で行う作業をまとめる。ソースコードだけでは設定できない項目を、公開前後に確認する。

## 公開前

- リポジトリの Visibility を Public にする前に、全履歴へ秘密情報、個人情報、実データの SQLite ファイルが含まれていないことを確認する。
- Description に「Local bulletin board for AI agents with SQLite, REST, SSE, MCP, and a read-only Web UI」など、ローカル専用であることが分かる説明を設定する。
- Website は必要になるまで空欄とし、Topics に `mcp`、`ai-agents`、`rust`、`sqlite` を設定する。
- `main` を既定ブランチにする。

## セキュリティ設定

- Settings > Code security and analysis で Private vulnerability reporting を有効にする。`SECURITY.md` の報告リンクが一般ユーザーから利用できることを確認する。
- Dependabot alerts と Dependabot security updates を有効にする。
- Actions の Workflow permissions は Read repository contents permission を既定にする。CI と Release ワークフローは通常 `contents: read` だけを使い、公開ジョブだけ `contents: write`、`id-token: write`、`attestations: write` を要求する。

## ブランチ保護

Rulesets または Branch protection rule で `main` に次を設定する。

- Pull Request を経由する。
- CI の `quality`、`Build x86_64-unknown-linux-gnu`、`Build aarch64-apple-darwin` を必須チェックにする。
- 必須チェックが完了するまでマージを禁止する。
- force push と branch deletion を禁止する。

個人開発でレビュー必須にすると緊急修正も自己承認できなくなるため、必須レビュー人数は共同メンテナー体制に合わせて決める。

## 公開後

- Issue Forms と Pull Request テンプレートが表示されることを確認する。
- 初回の Dependabot 実行が Cargo と GitHub Actions の両方を認識することを確認する。
- README のインストール URL と `SECURITY.md` の報告 URL が利用できることを確認する。
- CI 成功後に、macOS Apple Silicon と Linux x86_64 の成果物が Actions の artifact として作成されることを確認する。

## リリース

1. `Cargo.toml` の `version` をリリースするバージョンへ更新し、品質チェックをすべて通して `main` へ反映する。
2. Cargo のバージョンに `v` を付けたタグを作成し、GitHub へ push する。例: `v0.1.0`。
3. Release ワークフローがタグと Cargo のバージョン一致を確認し、全テスト後に2環境のアーカイブと SHA-256 ファイルを draft Release へ置く。Packslip の署名とアップロードが成功した場合だけ Release が公開される。Linux バイナリは Ubuntu 24.04 で作成するため、インストーラーと `packslip.toml` の glibc 2.39 以上という要件をビルド環境と同時に更新する。
4. 自動生成されたリリースノート、2つのアーカイブ、2つの SHA-256 ファイル、`packslip.sigstore.json` を確認する。
5. `mise use -g packslip:github.com/ta-dadadada/kakitsugi` で新規導入し、`mise skills ls` に `kakitsugi` が表示されることを確認する。

```sh
git tag -a v0.1.0 -m "Kakitsugi v0.1.0"
git push origin v0.1.0
```

公開ジョブが途中で失敗した場合、Release は draft のまま残る。一時的な障害なら同じワークフローを再実行でき、既存の draft 資産が更新される。コード修正が必要な場合は draft とタグを削除して修正コミットへタグを付け直す。公開後は既存資産やタグを変更せず、バージョンを上げて新しいリリースを作る。
