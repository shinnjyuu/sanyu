# 三语译（Sanyu Translate）

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md)

三语译是一个可通过 Docker 自托管的 Rust 离线翻译 API，使用 Firefox 的轻量模型在简体中文、英语和日语之间互译。

支持六个方向。中日互译经英语转译，运行时一次只加载一个方向的模型。

## 使用 Docker

```sh
docker build -t sanyu-translate .
docker run --rm -p 127.0.0.1:8080:8080 sanyu-translate
```

构建时下载四组 Firefox `base-memory` 模型和分词文件，校验解压后文件的 SHA-256。镜像包含这些文件，以非 root 用户运行，启动和翻译均可离线完成。同一 Docker 网络内的应用可以通过服务名调用接口。

启动后打开 `http://127.0.0.1:8080/`，即可从内置静态页面试译。

## HTTP API

`POST /translate` 接收 JSON。`from` 和 `to` 可用 `zh-CN`、`en`、`ja`。

```sh
curl -fsS http://127.0.0.1:8080/translate \
  -H 'Content-Type: application/json' \
  -d '{"from":"zh-CN","to":"ja","text":"更新完成后，请重启服务并检查网络连接。"}'
```

```json
{"translation":"更新が完了したら、サービスを再起動してネットワーク接続を確認してください。","status":"translated"}
```

`text` 最多 25,565 个 Unicode 字符，JSON 请求体最多 320 KiB。请求是同步的。服务一次处理一个翻译请求，另可排队一个，队列满时返回 `503`。长文可能需要一两分钟，耗时取决于 CPU、语种和文本。请按服务器的实测耗时设置客户端及反向代理的超时。

| HTTP | 含义 |
| --- | --- |
| `400` | 空白正文、同语言请求、非法语言或 JSON |
| `413` | 字符数或请求体超限 |
| `415` | 内容类型不是 JSON |
| `422` | 未生成有效译文，或检测到明显的原文残留 |
| `503` | 翻译队列已满，或翻译线程不可用 |

引擎按句处理长文，超长单句会按模型的 token 上限切分。无标点的长段落可能漏译，`translated` 表示服务已返回译文。

## 配置

| 环境变量 | 用途 | 默认值 |
| --- | --- | --- |
| `LISTEN_ADDR` | HTTP 监听地址 | `0.0.0.0:8080` |
| `MODEL_DIR` | 模型目录 | `/usr/share/sanyu/models` |
| `GLOSSARY_FILE` | UTF-8 JSON 术语表路径 | 未设置 |

替换模型时，可将目录只读挂载到 `MODEL_DIR`。目录应包含 `en-ja`、`en-zh`、`ja-en`、`zh-en` 四个子目录，各含 `model.bin` 和共用的 `vocab.spm`，或分开的 `source.spm` 和 `target.spm`。

也可按语言方向固定某个术语或短语的译文。出现重叠时优先使用较长的条目。模型分别翻译命中术语的前后文本。

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

用宿主机上的绝对路径替换示例中的源路径。配置在启动时读取。缺失的模型文件或格式无效的术语表会在启动时报告。

## 开发与许可

`cargo test --locked` 可测试接口与术语覆盖。直接运行 Rust 服务时，先用 `sh scripts/fetch-models.sh ./models` 下载模型，再设置 `MODEL_DIR=./models` 并运行 `cargo run --locked`。

三语译代码采用 MIT 许可。模型和依赖的许可见 [第三方声明](THIRD_PARTY_NOTICES.md)。
