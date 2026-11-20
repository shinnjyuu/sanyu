# Sanyu Translate

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md)

Sanyu Translate is a self-hosted Rust API that uses lightweight Firefox models to translate offline between Simplified Chinese, English, and Japanese in Docker.

All six directions are supported. Chinese and Japanese translate via English. The service loads one model at a time.

## Run with Docker

```sh
docker build -t sanyu-translate .
docker run --rm -p 127.0.0.1:8080:8080 sanyu-translate
```

The build downloads four sets of Firefox `base-memory` models and their tokenizer files, then verifies the SHA-256 digest of each unpacked file. The image bundles the files, runs as a non-root user, and works offline. Other apps on the same Docker network can reach the API by service name.

Open `http://127.0.0.1:8080/` after startup to try the built-in static test page.

## HTTP API

`POST /translate` accepts JSON with `from` and `to` set to `zh-CN`, `en`, or `ja`.

```sh
curl -fsS http://127.0.0.1:8080/translate \
  -H 'Content-Type: application/json' \
  -d '{"from":"zh-CN","to":"ja","text":"更新完成后，请重启服务并检查网络连接。"}'
```

```json
{"translation":"更新が完了したら、サービスを再起動してネットワーク接続を確認してください。","status":"translated"}
```

The `text` field is limited to 25,565 Unicode characters and the JSON body to 320 KiB. Requests are synchronous. The service handles one translation at a time, with room for one more in the queue. A full queue returns `503`. Documents can take a minute or two depending on the CPU, languages, and text. Set client and reverse proxy timeouts based on timings from your server.

| HTTP | Meaning |
| --- | --- |
| `400` | Blank text, identical source and target languages, invalid language codes, or invalid JSON |
| `413` | Character or request body limit exceeded |
| `415` | Content type other than JSON |
| `422` | Empty translation or obvious source-language text detected |
| `503` | Full translation queue or unavailable worker |

The engine translates sentences independently and splits very long sentences at the model's token limit. It may omit content in long passages without punctuation. A status of `translated` means the service returned a translation.

## Configuration

| Environment variable | Purpose | Default |
| --- | --- | --- |
| `LISTEN_ADDR` | HTTP listen address | `0.0.0.0:8080` |
| `MODEL_DIR` | Model directory | `/usr/share/sanyu/models` |
| `GLOSSARY_FILE` | Path to a UTF-8 JSON glossary | Unset |

To replace the models, mount a directory read-only and set `MODEL_DIR` to its path in the container. It must contain `en-ja`, `en-zh`, `ja-en`, and `zh-en` subdirectories. Each needs a `model.bin` and either a shared `vocab.spm` or separate `source.spm` and `target.spm` files.

The glossary lets you supply fixed translations for terms or phrases in a specific direction. Longer entries take precedence. The model translates the surrounding text separately.

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

Replace the example source path with an absolute path on the host. The service reads configuration at startup and reports missing model files or an invalid glossary.

## Development and license

Run `cargo test --locked` to test the API and glossary. To run the Rust service directly, download the models with `sh scripts/fetch-models.sh ./models`, then run `cargo run --locked` with `MODEL_DIR=./models`.

Sanyu Translate is MIT licensed. See the [third-party notices](THIRD_PARTY_NOTICES.md) for the model and dependency licenses.
