# Third-party notices

## Firefox Translations models and fxtranslate

The image includes four quantized `base-memory` models and their SentencePiece vocabulary files from Mozilla's Firefox Translations project. Mozilla distributes the model files under the Mozilla Public License 2.0 (MPL-2.0). The files in the image are the original unpacked files. Their downloads and SHA-256 digests are listed in [scripts/firefox-models.txt](scripts/firefox-models.txt).

The service runs these models using the MPL-2.0-licensed Rust engine `fxtranslate` 0.5.0. The [crate release](https://crates.io/crates/fxtranslate/0.5.0) contains the corresponding engine source. [Mozilla model registry](https://mozilla.github.io/translations/) · [Mozilla upstream license clarification](https://github.com/mozilla/translations/pull/1454) · [MPL-2.0 license](https://www.mozilla.org/MPL/2.0/)

## OpenCC

The service uses the MIT-licensed Rust library opencc-fmmseg by Laisuk Lai to normalize Simplified Chinese glossary entries. It embeds OpenCC lexicons distributed under Apache-2.0. Copyright © OpenCC contributors.

[opencc-fmmseg notices](https://github.com/laisuk/opencc-fmmseg/blob/master/THIRD_PARTY_NOTICES.md) · [OpenCC license](https://github.com/BYVoid/OpenCC/blob/master/LICENSE)

The Docker image includes license and notice files from the Rust dependencies under `/usr/share/sanyu/licenses/cargo/`.
