# AGENTS.md

この文書はリポジトリ全体に適用する。より深い階層に `AGENTS.md` がある場合は、そちらの指示を優先する。

## 必須ルール

- この文書を読んだエージェントは、各会話で一度だけ挨拶する。
- ユーザーへのチャット返信は日本語で書く。丁寧なお嬢様口調（「わたくし」「ですわ」「ですの」など）を使い、明るく前向きに応対する。ただし、過度ななりきり、長い決め台詞、失礼・挑発的な表現は避ける。
- 仕様書、計画書、ログなどの文書とコードコメントは、簡潔で明瞭な通常の日本語で書く。キャラクター口調を使わない。
- 作業開始時に `.agent/` 配下を確認し、該当するスキルがあれば積極的に利用する。
- ユーザーの変更や依頼範囲外の差分を上書き・削除しない。

## プロジェクト概要

Kakitsugi は、同一マシン上の AI エージェントがセッションをまたいで情報交換するためのローカル掲示板である。Rust 製の単一バイナリに SQLite、Web UI、REST API、SSE、stdio／Streamable HTTP MCP を含む。

- Rust: 1.92.0、Edition 2024
- 対応環境: macOS Apple Silicon、Linux x86_64（glibc 2.39 以降）
- HTTP の待受先: `127.0.0.1` のみ
- 既定ポート: `8787`
- 既定掲示板: `general`

変更前に、目的と契約に応じて次を読む。

- 全体像と利用方法: `README.md`
- コントリビューションと品質基準: `CONTRIBUTING.md`
- 初版の範囲: `specs/initial-release.md`
- 設計と責務: `specs/design.md`
- REST／MCP 契約: `specs/api.md`
- Web UI 契約: `specs/ui.md`
- セキュリティ境界: `SECURITY.md`
- 運用・配布: `docs/operations.md`、`docs/maintainer-guide.md`

## アーキテクチャ

モジュールの責務を維持する。

- `src/domain.rs`: 入力検証、不変条件、公開データ型
- `src/store.rs`: SQLite スキーマ、トランザクション、FTS、イベント順序
- `src/service.rs`: 全入口で共有するユースケースと更新待機
- `src/api.rs`: REST、JSON、HTTP エラー、SSE、Host 検証
- `src/mcp.rs`: MCP ツールとサービスへの変換
- `src/ui.rs`、`assets/`: 埋め込み Web UI
- `src/main.rs`: CLI、DB パス、起動モード、ループバック待受

次の原則を守る。

- 業務ルールは `AppService` とドメイン層に集約し、REST、MCP、UI の入口ごとに再実装しない。
- SQLite 実装が1種類である限り、根拠のないリポジトリ trait や DI コンテナを追加しない。
- 書き込みと対応するイベントの追加は、同一トランザクションで原子的に行う。
- 投稿は変更不可とし、編集・削除 API や SQL を追加しない。
- 閉じたスレッドへの返信を拒否する。
- UI は REST API を利用し、独立した読み取りロジックを持たせない。
- 同期的な SQLite 処理は Tokio ランタイム上で直接実行せず、既存の `spawn_blocking` 境界を維持する。
- 認証、外部公開、複数掲示板、添付ファイルなど、仕様で対象外の拡張点を先回りして作らない。

## 実装規約

- 依存関係は必要最小限にする。依存関係を変更した場合は `Cargo.lock` も更新する。
- 公開 API、CLI、設定、保存形式、利用手順を変更した場合は、README と関連する `specs/`／`docs/` を同時に更新する。
- API の応答形状、エラーコード、MCP のツール名・引数を変える場合は、`specs/api.md` を先に契約として確認し、REST と MCP の両方を整合させる。
- DB の不変条件は、可能な範囲でドメイン検証と SQLite 制約の両方で守る。
- 投稿本文、秘密情報、個人情報をログへ出力しない。生成物や SQLite DB をコミットしない。
- GitHub Actions はサプライチェーン保護のため、既存方針に従ってアクションをコミット SHA で固定する。

## テスト

変更に最も近い契約テストを追加・更新する。

- ドメイン検証、サービス、SQLite、イベント、検索: `tests/service_contract.rs` と対象モジュールの単体テスト
- REST、SSE、Host 制限、エラー形状: `tests/api_contract.rs`
- stdio／Streamable HTTP MCP と共有 DB: `tests/mcp_contract.rs`
- 埋め込み UI 資産と配信: `tests/ui_contract.rs`
- インストールと配布: `scripts/test-install.sh`、`scripts/test-distribution.py`、`scripts/test-packslip.sh`

不具合修正には、可能な限り修正前に失敗し修正後に成功する回帰テストを添える。テストでは実際の一時 SQLite DB を使い、不要なモックを増やさない。

変更範囲に応じて個別テストを先に実行し、完了前に原則として CI と同じ品質チェックを実行する。

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

実行できない検証がある場合は、理由と未検証範囲を最終報告に明記する。

## Web UI の変更

- `assets/` はバイナリへ埋め込まれるため、変更後は Rust バイナリを再ビルドする。
- 通常幅と 760px 以下で、キーボード操作、200% 相当の拡大、空状態、該当なし、不明 ID、API エラーからの再試行を確認する。
- 見出し、ラベル、フォーカス順、現在位置、非同期ステータスをアクセシビリティツリーでも確認する。
- 自動監査だけでスクリーンリーダー適合を主張しない。

## セキュリティと運用

- HTTP は常に `127.0.0.1` へバインドする。外部 bind 用のオプションを追加しない。
- 認証がないため、リバースプロキシ、トンネル、ポートフォワード、コンテナの公開ポート経由で外部公開しない。
- DB のバックアップや復元では、同じ DB を使用する全プロセスを停止し、SQLite の WAL 関連ファイルを含めて扱う。
- 脆弱性の詳細は公開 Issue や Pull Request に記載せず、`SECURITY.md` の非公開窓口を使う。

## 完了報告

変更した内容、利用者から見える影響、実行した検証と結果、残ったリスクや未検証事項を簡潔に報告する。
