# tympan-ladspa

> Source: README.md @ fc6de118ab08317e35a9e2c67dea7a1d5c77428b
>
> [English](./README.md)

LADSPA プラグインを Rust で記述するためのフレームワーク。

`tympan-ladspa` は LADSPA（Linux Audio Developer's Simple Plugin API）の C インタフェースを Rust から扱うための抽象化を提供します。これにより、PipeWire の filter-chain や Ardour などの LADSPA 対応ホストで動作するオーディオプラグインを Rust で実装できます。

## ステータス

**意図した機能はすべて実装済みです。** [`docs/overview.md`](docs/overview.md) の「In scope」項目はすべて実装され、[ADR 0005](docs/decisions/0005-ci-verification-strategy.md)（段階的な CI 計画）のうち保留とした項目を除くすべてが CI に組み込まれています:

### フレームワーク

- Layer 1 — 低レベルの LADSPA FFI（[`src/raw/`](src/raw/)）。
- Layer 2 — リアルタイム向けプリミティブ（[`src/realtime/`](src/realtime/)）。型レベルのマーカー [`RealtimeContext`](src/realtime/context.rs)、ロックフリーの [SPSC リングバッファ](src/realtime/ring.rs)、そして「リアルタイムスレッドからログを記録し、別スレッドで取り出す」パターンをまとめた [`LogSink`](src/realtime/log.rs) ラッパー。
- Layer 3 — プラグイン作者向け API。[`Plugin`](src/plugin.rs) トレイト、`PortDescriptor` ビルダーを持つ [`Ports`](src/port.rs)、そして LADSPA の `ladspa_descriptor` エントリポイントをエクスポートする宣言的マクロ [`plugin_entry!`](src/macros.rs)。

### サンプル

[`examples/`](examples/) 配下に 3 つのリファレンスプラグインがあります。

- [`gain/`](examples/gain/) — 最小構成の線形ゲイン。アロケーションフリー（alloc-free）の不変条件を CI で検証しています。
- [`noise-gate/`](examples/noise-gate/) — ヒステリシス付きノイズゲート。複数のコントロール入力とインスタンスごとの状態を示す例です。
- [`delay/`](examples/delay/) — フィードバック付きディレイ。`LogSink` をエンドツーエンドで使う例です。

### CI

| Tier | チェック内容 |
|---|---|
| 1 | `cargo fmt --check`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`、`stable` と MSRV `1.75` での `cargo build` / `cargo test`、`nm -D` によるシンボル検査 |
| 2 | `ladspa-sdk` 連携。各サンプルに対する `analyseplugin`（メタデータのラウンドトリップ検査）と `applyplugin`（ライフサイクル全体と数値検証）。`Plugin::run` に対して `CLAUDE.md` の禁止事項 1 を検証する `assert_no_alloc` 統合テスト。ワークスペース全体での AddressSanitizer（nightly + `-Zbuild-std`） |
| 3 | ThreadSanitizer（nightly + `-Zbuild-std`）。5 万件の並行交換で SPSC リングバッファの Acquire/Release 順序を検証。`cargo-deny` によるサプライチェーン検査（advisories、licences、bans、sources） |

このフレームワークはすでに実用できます。`cdylib` クレートに `impl Plugin for MyPlugin` と `tympan_ladspa::plugin_entry!(MyPlugin)` を書けば、生成された `.so` を任意の LADSPA ホストで読み込めます。最小のレシピは [`examples/gain/`](examples/gain/) にあります。パニック戦略、`UNIQUE_ID`、シンボルの可視性、リアルタイムのデバッグ、よくある落とし穴などの実践的なレシピは [`docs/plugin-author-guide.md`](docs/plugin-author-guide.md) にまとめています。

### 今後の作業

ADR 0005 の Tier 3 のうち、まだ CI に入っていない 2 項目は、ADR 本文で保留として記録されています。一つは `strace` によるシステムコールの許可リストで、glibc やカーネルの更新で壊れやすいためです。もう一つは `criterion` による回帰ベンチマークです。リアルタイム経路の基準値がまだなく、現在の `run()` が単純なスライスのイテレータにすぎないためです。どちらも再評価の契機（トリガー）が [ADR 0005 § Trigger for revisiting](docs/decisions/0005-ci-verification-strategy.md) に明記されています。

`plugin_entry!` の複数プラグイン向け `cdylib` 版は、[`src/macros.rs`](src/macros.rs) のドキュメントコメントに概略がありますが、リポジトリ内にそれを必要とする利用者が現在いないため、未実装です。

## 名前について

*Tympan* は蛾の鼓膜器官（tympanal organ）を指します。メイガ科およびヤガ科の蛾の腹部にある、膜ベースの超音波センサです。コウモリの反響定位（エコーロケーション）の音を検出するために進化しました。この名前はライブラリの役割を表しています。ホストのオーディオエンジンとユーザー空間の Rust コードのあいだにある、薄い膜です。

## プラグイン作者向けクイックスタート

このフレームワークに依存する新しい `cdylib` クレートを作り、プラグインを宣言して、エントリポイントのマクロを呼び出します。

```toml
# Cargo.toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
tympan-ladspa = "0.1"
```

```rust
// src/lib.rs
use tympan_ladspa::{
    plugin_entry,
    port::{PortDefault, PortDescriptor, Ports},
    realtime::RealtimeContext,
    InstantiateError, Plugin,
};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    const UNIQUE_ID: u32 = 0x_1234_5678;       // ladspa.org で取得
    const LABEL: &'static str = "my_plugin";
    const NAME: &'static str = "My Plugin";
    const MAKER: &'static str = "your name";
    const COPYRIGHT: &'static str = "MIT OR Apache-2.0";

    fn ports() -> &'static [PortDescriptor] {
        static PORTS: &[PortDescriptor] = &[
            PortDescriptor::audio_input("In"),
            PortDescriptor::audio_output("Out"),
            PortDescriptor::control_input("Gain")
                .with_default(PortDefault::One)
                .with_bounds(0.0, 4.0),
        ];
        PORTS
    }

    fn instantiate(_sample_rate: u32) -> Result<Self, InstantiateError> {
        Ok(Self)
    }

    fn run(&mut self, _rt: &RealtimeContext, _frames: usize, ports: &mut Ports<'_>) {
        let gain = ports.control_input(2);
        let (input, output) = ports.audio_in_out(0, 1);
        for (i, o) in input.iter().zip(output.iter_mut()) {
            *o = *i * gain;
        }
    }
}

plugin_entry!(MyPlugin);
```

`cargo build --release` でビルドし、`target/release/libmy_plugin.so` を `~/.ladspa/` にコピーします。同じ構成の完全なクレートは [`examples/gain/`](examples/gain/) を参照してください。

## 開発

プロジェクトの CI は `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test` を実行します。さらに `examples` ジョブでは、LADSPA SDK を通じて各サンプルプラグインをエンドツーエンドで動かします（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)）。段階的な検証戦略は [ADR 0005](docs/decisions/0005-ci-verification-strategy.md) に記録しています。

`git push` の前に同じ fmt と clippy のチェックをローカルでも実行するには、リポジトリの pre-push フックを有効にします。

```sh
git config core.hooksPath .githooks
```

フックの本体は [`.githooks/pre-push`](.githooks/pre-push) です。push する範囲に `*.rs`、`Cargo.toml`、`Cargo.lock` の変更が含まれていなければ何もしないため、ドキュメントだけの push が遅くなることはありません。一度だけ回避するには `git push --no-verify` を使います。

## ライセンス

以下のいずれかのライセンスの下で提供されます。

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE) または <http://www.apache.org/licenses/LICENSE-2.0>）
- MIT license（[LICENSE-MIT](LICENSE-MIT) または <http://opensource.org/licenses/MIT>）

利用者の選択により、いずれを選んでも構いません。

### コントリビューション

別段の明示がない限り、Apache-2.0 ライセンスで定義されるとおり、貢献者が本作品への組み込みのために意図的に提出したコントリビューションは、上記のとおりデュアルライセンスされるものとし、追加の条項や条件は一切伴わないものとします。

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [`docs/overview.md`](docs/overview.md) | プロジェクトの目的、スコープ、既存実装との比較 |
| [`docs/architecture.md`](docs/architecture.md) | API 設計とモジュール構成 |
| [`docs/plugin-author-guide.md`](docs/plugin-author-guide.md) | フレームワーク上でプラグインを書くための実践的なレシピ |
| [`docs/references.md`](docs/references.md) | LADSPA 仕様、PipeWire 連携、先行事例 |
| [`docs/decisions/`](docs/decisions/) | アーキテクチャ決定記録（ADR） |
| [`docs/handoff-protocol.md`](docs/handoff-protocol.md) | 長期作業のためのセッション引き継ぎプロトコル |

## サンプル

| サンプル | 説明 |
|---|---|
| [`examples/gain/`](examples/gain/) | 最小構成の線形ゲインプラグイン。フレームワークを使う最小の例です。alloc-free を CI で検証しています。 |
| [`examples/noise-gate/`](examples/noise-gate/) | ヒステリシス付きノイズゲート。複数のコントロール入力とインスタンスごとの状態を示します。単体テストしやすいように、DSP のロジックを純粋関数に切り出しています。 |
| [`examples/delay/`](examples/delay/) | フィードバック付きディレイライン。`instantiate` で `Vec<f32>` のリングバッファを事前に確保し、`activate` で状態をリセットします。三つのコントロールポートを使い、`audio_in_out` という例外的な経路も示します。 |
