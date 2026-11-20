# 三語訳（Sanyu Translate）

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md)

三語訳は、Firefox の軽量モデルで簡体字中国語・英語・日本語をオフラインで相互翻訳する Rust 製 API で、Docker でセルフホストできます。

3 言語間の 6 方向に対応しています。中国語と日本語の間は英語を経由し、一度に 1 つのモデルを読み込みます。

## Docker で起動

```sh
docker build -t sanyu-translate .
docker run --rm -p 127.0.0.1:8080:8080 sanyu-translate
```

ビルド時に Firefox の `base-memory` モデルとトークナイザーのファイルを 4 組ダウンロードし、展開後の各ファイルの SHA-256 を検証します。ファイルはイメージに同梱されます。コンテナは非 root ユーザーで動作し、オフラインで起動・翻訳できます。同じ Docker ネットワーク内のアプリはサービス名で API に接続できます。

起動後に `http://127.0.0.1:8080/` を開くと、組み込みの静的ページで翻訳を試せます。

## HTTP API

`POST /translate` に JSON を送信します。`from` と `to` には `zh-CN`、`en`、`ja` を指定できます。

```sh
curl -fsS http://127.0.0.1:8080/translate \
  -H 'Content-Type: application/json' \
  -d '{"from":"zh-CN","to":"ja","text":"更新完成后，请重启服务并检查网络连接。"}'
```

```json
{"translation":"更新が完了したら、サービスを再起動してネットワーク接続を確認してください。","status":"translated"}
```

`text` は Unicode で最大 25,565 文字、JSON のリクエスト本文は最大 320 KiB です。リクエストは同期処理です。一度に 1 件を翻訳し、もう 1 件を待ち行列に保持します。待ち行列が満杯の場合は `503` を返します。長文は CPU、言語、内容によって 1〜2 分かかる場合があります。サーバーでの実測に合わせて、クライアントとリバースプロキシのタイムアウトを設定してください。

| HTTP | 意味 |
| --- | --- |
| `400` | 空白のみの本文、同一言語の指定、無効な言語コードまたは JSON |
| `413` | 文字数またはリクエスト本文の上限超過 |
| `415` | JSON 以外のコンテンツタイプ |
| `422` | 訳文が空、または原文の言語が明らかに残っている |
| `503` | 待ち行列が満杯、または翻訳ワーカーが利用できない |

長文は文ごとに翻訳され、長すぎる文はモデルのトークン上限で分割されます。句読点のない長い文章では訳抜けが起こる場合があります。`translated` はサービスが訳文を返したことを示します。

## 設定

| 環境変数 | 用途 | 既定値 |
| --- | --- | --- |
| `LISTEN_ADDR` | HTTP の待ち受けアドレス | `0.0.0.0:8080` |
| `MODEL_DIR` | モデルのディレクトリ | `/usr/share/sanyu/models` |
| `GLOSSARY_FILE` | UTF-8 の JSON 用語集へのパス | 未設定 |

モデルを差し替えるには、ディレクトリを読み取り専用でマウントし、コンテナ内のパスを `MODEL_DIR` に指定します。`en-ja`、`en-zh`、`ja-en`、`zh-en` のサブディレクトリが必要です。それぞれに `model.bin` と、共通の `vocab.spm` または個別の `source.spm` と `target.spm` を配置します。

用語集では翻訳方向ごとに用語やフレーズの訳文を指定できます。重複する場合は長い項目を優先します。一致した用語の前後のテキストは、モデルで個別に翻訳します。

```json
[
  {"from":"en","to":"ja","text":"Sanyu Translate","translation":"三語訳"}
]
```

```sh
docker run --rm -p 127.0.0.1:8080:8080 \
  --mount "type=bind,source=/absolute/path/glossary.json,target=/glossary.json,readonly" \
  -e GLOSSARY_FILE=/glossary.json sanyu-translate
```

例のマウント元はホスト上の絶対パスに置き換えてください。設定は起動時に読み込みます。モデルのファイルが見つからない場合や用語集の形式が無効な場合は、起動時にエラーを表示します。

## 開発とライセンス

`cargo test --locked` で API と用語集のテストを実行できます。ソースから起動する場合は、`sh scripts/fetch-models.sh ./models` でモデルを取得し、`MODEL_DIR=./models` を設定してから `cargo run --locked` を実行します。

三語訳のコードは MIT ライセンスで提供されます。モデルと依存ライブラリのライセンスは [サードパーティーのライセンス表記](THIRD_PARTY_NOTICES.md) を参照してください。
