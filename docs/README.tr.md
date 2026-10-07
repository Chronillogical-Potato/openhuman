<h1 align="center">OpenHuman</h1>

<p align="center">
 <img src="../gitbooks/.gitbook/assets/demo.png" alt="The Tet" />
</p>

<p align="center" style="display: inline-block">
	<a href="https://trendshift.io/repositories/23680" target="_blank" style="display: inline-block">
		<img src="https://trendshift.io/api/badge/repositories/23680" alt="tinyhumansai%2Fopenhuman | Trendshift" style="width: 250px; height: 55px;" width="250" height="55"/>
	</a>
	<a href="https://www.producthunt.com/products/openhuman?embed=true&amp;utm_source=badge-top-post-badge&amp;utm_medium=badge&amp;utm_campaign=badge-openhuman" target="_blank" rel="noopener noreferrer">
		<img alt="OpenHuman - An open source AI harness built with the human in mind | Product Hunt" width="250" height="54" src="https://api.producthunt.com/widgets/embed-image/v1/top-post-badge.svg?post_id=1136902&amp;theme=light&amp;period=daily&amp;t=1778916022823">
		</a>
		<a href="https://www.producthunt.com/products/openhuman?embed=true&amp;utm_source=badge-top-post-badge&amp;utm_medium=badge&amp;utm_campaign=badge-openhuman" target="_blank" rel="noopener noreferrer">
			<img alt="OpenHuman - An open source AI harness built with the human in mind | Product Hunt" width="250" height="54" src="https://api.producthunt.com/widgets/embed-image/v1/top-post-badge.svg?post_id=1136902&amp;theme=light&amp;period=weekly&amp;t=1779351403565">
		</a>
</p>
<p align="center" style="display: inline-block">
 <a href="https://www.producthunt.com/products/openhuman?embed=true&amp;utm_source=badge-top-post-topic-badge&amp;utm_medium=badge&amp;utm_campaign=badge-openhuman" target="_blank" rel="noopener noreferrer">
  <img alt="OpenHuman - An open source AI harness built with the human in mind | Product Hunt" width="250" height="54" src="https://api.producthunt.com/widgets/embed-image/v1/top-post-topic-badge.svg?post_id=1136902&amp;theme=light&amp;period=weekly&amp;topic_id=268&amp;t=1779351808756">
  </a>
  <a href="https://www.producthunt.com/products/openhuman?embed=true&amp;utm_source=badge-top-post-topic-badge&amp;utm_medium=badge&amp;utm_campaign=badge-openhuman" target="_blank" rel="noopener noreferrer">
   <img alt="OpenHuman - An open source AI harness built with the human in mind | Product Hunt" width="250" height="54" src="https://api.producthunt.com/widgets/embed-image/v1/top-post-topic-badge.svg?post_id=1136902&amp;theme=light&amp;period=weekly&amp;topic_id=46&amp;t=1779351808756">
   </a>
 </p>

<p align="center">
 <strong>Rust çekirdekli, açık kaynaklı bir ajan altyapısı: hafif, modüler ve hâlihazırda kullandığınız LLM, bellek ya da arama motoruna takılabilir.</strong>
</p>

<p align="center">
 <a href="https://github.com/tinyhumansai/openhuman/discussions">Tartışmalar</a> •
 <a href="https://guild.tinyhumans.ai/">Discord</a> •
 <a href="https://www.reddit.com/r/tinyhumansai/">Reddit</a> •
 <a href="https://x.com/intent/follow?screen_name=tinyhumansai">X/Twitter</a> •
 <a href="https://tinyhumans.gitbook.io/openhuman/">Belgeler</a> •
 <a href="https://x.com/intent/follow?screen_name=senamakel">@senamakel'i takip edin (Geliştirici)</a>
</p>

<p align="center">
  🇺🇸 <a href="../README.md">English</a> | 🇨🇳 <a href="./README.zh-CN.md">简体中文</a> | 🇯🇵 <a href="./README.ja-JP.md">日本語</a> | 🇰🇷 <a href="./README.ko.md">한국어</a> | 🇩🇪 <a href="./README.de.md">Deutsch</a> | 🇹🇷 <a href="./README.tr.md">Türkçe</a> | 🇵🇰 <a href="./README.ur-pk.md">اردو</a>
</p>

<p align="center">
 <img src="https://img.shields.io/badge/status-early%20beta-orange" alt="Erken Beta" />
 <a href="https://github.com/tinyhumansai/openhuman/releases/latest"><img src="https://img.shields.io/github/v/release/tinyhumansai/openhuman?label=latest" alt="Son Sürüm" /></a>
 <a href="https://github.com/tinyhumansai/openhuman/stargazers"><img src="https://img.shields.io/github/stars/tinyhumansai/openhuman?style=flat" alt="GitHub Yıldızları" /></a>
 <a href="../LICENSE"><img src="https://img.shields.io/github/license/tinyhumansai/openhuman" alt="Lisans" /></a>
</p>

> **Erken Beta**: Aktif olarak geliştiriliyor. Bazı pürüzlerle karşılaşabilirsiniz.

> OpenHuman, yayınlandıktan sonraki bir hafta içinde art arda dokuz gün boyunca GitHub'da bir numaralı trend depo oldu.

# Kurulum

Yükleyicileri [tinyhumans.ai/openhuman](https://tinyhumans.ai/openhuman?utm_source=github&utm_medium=readme) adresinden ya da [GitHub Releases](https://github.com/tinyhumansai/openhuman/releases/latest) sayfasından indirin.

Terminalden kurulum (Homebrew, Debian/Ubuntu `.deb`, AUR, kurulum betikleri ve platform notları) için **[INSTALL.md](../INSTALL.md)** dosyasına bakın.

# OpenHuman nedir?

OpenHuman; etrafında bir masaüstü uygulaması, bir tarayıcı arayüzü, bir terminal istemcisi ve bir Rust kütüphanesi bulunan bir Rust çekirdeğidir. Dördünü de aynı çekirdek çalıştırır. Aşağıdaki her bölüm, [belgelerdeki](https://tinyhumans.gitbook.io/openhuman/) ayrıntılı açıklamaya bağlantı verir.

## Hafif ve hızlı

Çekirdek, arayüzün bir soket üzerinden konuştuğu ayrı bir arka plan hizmeti olarak değil, aynı süreç içinde çalışır. Tek bir süreçte 50, 100 ve 500 canlı ajanla yapılan bir [filo ölçümü](../gitbooks/developing/performance.md), her ek ajan için 1.985, 1.866 ve 1.770 KiB marjinal maliyet ölçtü; toplam bellek 223 MiB, 356 MiB ve 1.393 MiB'de dengelendi. Aynı iş yükü tek süreçte 500 ajan yerine 500 ayrı süreç olarak çalıştırıldığında örnek başına yaklaşık 48 MiB tutuyor; yani tek süreci paylaşmak kabaca 25 kat daha yoğun. Tek makinede binlerce ajan hedeflenen yön; henüz ulaşılmış bir rakam değil.

Soğuk bir ajan turu 102 ms sürer; dokuz aşamalı tam başlatma (yapılandırma yükleme, kayıt başlatma, ajan oluşturma, bellek kurulumu, ilk tur) 476 ms sürer. İnce bir derleme yaklaşık 42 MiB RSS'te dengelenir; bunun kabaca 15,2 MiB'i özel heap, geri kalanı belleğe yüklenmiş çalıştırılabilir kod ve ayırıcı ek yüküdür. [Token sıkıştırma](../gitbooks/features/token-compression.md) (tinyjuice) modele gerçekten ulaşan içeriği de azaltır; böylece büyük bir bağlam, ham boyutunun düşündürdüğünden daha az maliyetlidir.

Tüm yöntem ve rakamlar: [`docs/library-benchmarking.md`](./library-benchmarking.md), [`docs/harness-comparison-2026-07-22.md`](./harness-comparison-2026-07-22.md) ve [performans](../gitbooks/developing/performance.md).

## Modüler

Neyin derlemeye gireceğini Cargo özellik kapıları (feature gates) belirler. Katkıcı varsayılanı dokuz kapıdır (`media`, `skills`, `flows`, `mcp`, `channels`, `http-server`, `scheduler-gate`, `file-logging`, `modules`); dağıtılan masaüstü ürünü ise `scripts/ci/product-features.txt` içinde listelenen daha geniş bir kümeyi açar. Hepsini kapatmak, sembolleri ayıklanmış hâlde 51 MiB'lik saf ince bir derleme verir; gömme için önerilen tarif olan `skills` ve `flows` geri eklendiğinde boyut yaklaşık 60 MiB olur; tüm kapılar açıldığında sembolleri ayıklanmamış 116 MiB'lik bir ikili dosya üretilir. `scripts/kernel-floor.sh`, alt sınırın yeniden yükselmemesi için bağımlılık sayısını yalnızca aşağı yönlü bir mandalla sınırlar.

Derleme zamanının ötesinde yetenekler, yüklenebilir yerel modüllerden gelir: `tinydocs`, `tinyvoice`, `tinyjuice`, `tinyruntime`, `tinywallet`, `tinymcp`, `tinychannels` ve `tinyconnectors`. Her birinin, arayüzünü ve iletişim türlerini tanımlayan küçük bir `*-bus` sözleşme crate'i vardır. Her kapının ölçülmüş ödünleşimleri için [`docs/library-minimal-recipe.md`](./library-minimal-recipe.md) dosyasına bakın.

## Takılabilir motorlar

OpenHuman'ın çağırdığı her motor koda gömülü değildir, yapılandırmayla seçilir:

- LLM: yönetilen TinyHumans rotası, Ollama, LM Studio, MLX, OpenAI uyumlu herhangi bir yerel sunucu, Claude Code ya da Claude Agent SDK ve OpenRouter, OpenAI, Anthropic, Google, Groq, Mistral, DeepSeek, Together ve Fireworks dahil kendi anahtarınızı getirebileceğiniz (BYOK) 26 sağlayıcı. Bkz. [yerel modeller ve BYOK](https://tinyhumans.gitbook.io/openhuman/features/model-routing/local-and-byok-models).
- Embedding: Voyage destekli yönetilen rota ya da kendi Voyage, OpenAI, Cohere, Ollama veya OpenAI uyumlu uç noktanız.
- Bellek: [Memory v2](https://tinyhumans.gitbook.io/openhuman/features/memory), takılabilir bir motor üzerinde Hatırlama, Getirme ve Saklama işlemlerini sunar: TinyHumans (hesabınızla oturum açılan barındırılan CortexDB) ya da kendi CortexDB'niz (uç nokta ve anahtar). Belgelerden (klasörler, dosyalar, bağlantılar, GitHub, RSS, bağlı uygulamalar), her ajanın sohbetlerinden ve paylaşılan öğrenilenlerden oluşan ortak bir beyin tutar; her turdan önce önemli olanı hatırlar ve soruları kaynak göstererek yanıtlar. Hiçbir motor seçilmezse bellek kapalıdır. Tüm ayarlar Bağlantılar > Bellek altında yapılır.
- Web araması: abonelikle gelen yönetilen arama ya da Parallel, Brave, Querit, Exa, Tavily veya kendi barındırdığınız bir SearXNG sunucusu için kendi anahtarınız.

Motor ayrıntıları: [motorlar](../gitbooks/developing/engines.md).

## Jev hızlı karar verir

Her karar, modelin metin üretmesini gerektirmez. [Jev](../gitbooks/developing/jev.md), TinyHumans System One proxy'si üzerinden çalışan küçük bir karar modelidir. Bir soruyu ve sabit bir seçenek kümesini alır ve her biri için kalibre edilmiş bir olasılık döndürür: bunlardan birini seç (Choice), bunu puanla (Score) ya da evet/hayır (Noul). Asla düz metin yazmaz.

En net kullanım alanı [araç aramasıdır](./plans/jev-tool-search-baseline.md): masada 215 çekirdek araç ve 1.000 Composio eylemi, elde 160 test isteği varken düz BM25 erişimi doğru aracı ilk tercihinde yalnızca %22,5 oranında buldu ve araç gerektirmeyen 31 isteğin 26'sında gereksiz araç çağrısı yaptı. Embedding ile en iyi 20 adayı getirip aralarından Jev'in seçmesine izin vermek, doğru aracı %62,0 oranında buldu (ilk 3'te %66,7) ve yalnızca 1 gereksiz çağrı yaptı; p50 gecikmesi BM25'in 28 milisaniyesine karşı 1,5 saniyeydi. Jev'in önce Composio uygulama ailesini, ardından o aile içindeki eylemi seçmesine izin vermek, yalnızca Composio doğruluğunu ilk tercihte %80,3'e çıkarıyor. TinyHumans kimlik bilgisi yoksa otomatik olarak BM25'e geri döner.

Jev, [tarayıcı aracı](../crates/openhuman-core/src/modules/browser_task.rs) içindeki adım adım kararları da yönetir; sonuç doğuran bir eylem (satın alma, gönderme, silme) doğrudan yürütülmek yerine `NeedsConfirmation` döndürür.

## İş akışları

[İş akışları](../gitbooks/features/workflows.md), açık kaynaklı [tinyflows](https://github.com/tinyhumansai/tinyflows) motoru üzerine kurulu, kaydedilmiş ve türlendirilmiş otomasyon graflarıdır. Katalogda 22 düğüm türü bulunur (ajan çağrıları, HTTP istekleri, kod, koşullar, döngüler, alt iş akışları, onaylar ve daha fazlası). Bir graf bir zamanlamayla, bir uygulama olayıyla ya da elle tetiklenebilir ve bir duraklamanın ardından çalışmanın ortasından devam edebilir.

<p align="center">
 <img src="../gitbooks/.gitbook/assets/workflows.png" alt="OpenHuman iş akışı tuvali">
</p>

> İş akışını ajan önerir; siz onu bir tuval üzerinde inceleyip kaydedersiniz.

n8n veya Zapier'den farkı şu: Ne istediğinizi anlatırsınız, ajan grafın taslağını çıkarır, siz de düğümleri elle bağlamak yerine onu inceleyip kaydedersiniz.

## Masaüstü, tarayıcı ve terminal

Aynı çekirdek üç biçimde sunulur: Windows, macOS ve Linux için Tauri v2 ve Wry tabanlı bir masaüstü uygulaması; herhangi bir tarayıcıda çalışan aynı SPA (`pnpm dev:app:web`); ve `ratatui` tabanlı bir terminal istemcisi (`crates/openhuman-tui`).

## Bir Rust kütüphanesi

`openhuman-embed`, çekirdeği doğrudan başka bir Rust sürecine gömmek için türlendirilmiş arayüzdür: süreç başına bir `Runtime`, ardından bunun üzerinde her birinin kendi sağlayıcısı, erişim düzeyi, çalışma dizini, MCP sunucuları, yetenekleri, istemi ve korumalı alanı olan istediğiniz sayıda bağımsız `Agent`. Aşağıdaki kod [`crates/openhuman-embed/README.md`](../crates/openhuman-embed/README.md) dosyasındakiyle birebir aynıdır:

```rust
use openhuman_embed::{Access, AgentSpec, McpServer, Provider, Runtime, Workspace};

let runtime = Runtime::builder()
    .workspace(Workspace::dir("/var/lib/my-product/openhuman"))
    .api_key("th_live_…")                     // the only credential in library mode
    .build()
    .await?;

let reviewer = runtime.agent(
    AgentSpec::new("reviewer")
        .system_prompt("You review pull requests and never edit files.")
        .access(Access::readonly())
        .skills_dir("./skills/review")        // copied into this agent's own skills root
        .action_dir("/srv/checkouts/pr-42"),
)?;

let fixer = runtime.agent(
    AgentSpec::new("fixer")
        .provider(Provider::openai_compatible("https://api.example/v1", "sk-…").model("gpt-5"))
        .access(Access::full())
        .mcp(McpServer::stdio("github", "gh-mcp", ["stdio"]))
        .action_dir("/srv/checkouts/pr-42"),
)?;

let review = reviewer.run("Summarise the risks in this change.").await?;
let fix = fixer
    .turn(format!("Address these findings:\n{}", review.reply))
    .send()
    .await?;
println!("{}", fix.reply);

// Continue a conversation with the same agent.
let again = fixer.turn("Now run the tests.").session(&fix.session_id).send().await?;
println!("{}", again.reply);
```

Ayrıntılar, özellik bayraklarının aktarımı ve en küçük ayak izi tarifi: [gömme](../gitbooks/developing/embedding.md).

## Her şey için tek bir TinyHumans API anahtarı

Tek bir TinyHumans API anahtarı; yönetilen LLM çıkarımını (OpenRouter model kataloğuna erişim dahil), web aramasını, embedding'leri, medya üretimini, entegrasyonları, sesi ve Jev sıralayıcısını kapsar. Anahtarı bir kez, kodda (`.api_key("th_...")`) ya da arayüzsüz bir sunucu için `OPENHUMAN_BACKEND_API_KEY` olarak verin; bu hizmetlerin hepsi kullanıma hazır olur. Ayrıntılar: [TinyHumans API anahtarı](../gitbooks/developing/tinyhumans-api-key.md).

## Açık kaynak

OpenHuman, [GPL-3.0](../LICENSE) lisansı altındadır.

## OpenHuman ve diğer ajan altyapıları

Genel bir karşılaştırma (ürünler gelişiyor, her sağlayıcıyla ayrıca doğrulayın). OpenHuman; **sağlayıcı dağınıklığını en aza indirmek**, **iş akışı bilgisini cihazda tutmak** ve ajana yalnızca sohbetin değil, verilerinizin de **kalıcı bir belleğini** vermek için tasarlandı.

|                          | Claude Cowork          | OpenClaw             | Hermes Agent         | OpenHuman                                                                                         |
| ------------------------ | ---------------------- | -------------------- | -------------------- | ------------------------------------------------------------------------------------------------- |
| **Açık kaynak**          | 🚫 Tescilli            | ✅ MIT               | ✅ MIT               | ✅ GNU                                                                                            |
| **Kolay başlangıç**      | ✅ Masaüstü + CLI      | ⚠️ Önce terminal     | ⚠️ Önce terminal     | ✅ Sade arayüz, dakikalar içinde                                                                  |
| **Maliyet**              | ⚠️ Abonelik + eklentiler | ⚠️ Kendi modelleriniz | ⚠️ Kendi modelleriniz | ✅ Tek abonelik + TokenJuice                                                                      |
| **Bellek**               | ✅ Sohbetle sınırlı    | ⚠️ Eklentiye bağımlı | ✅ Kendi kendine öğrenen | 🚀 Takılabilir motor (barındırılan TinyHumans ya da kendi CortexDB'niz), her turdan önce hatırlama, kaynak gösterme |
| **Entegrasyonlar**       | ⚠️ Az bağlayıcı        | ⚠️ Kendiniz getirin  | ⚠️ Kendiniz getirin  | 🚀 100+ OAuth · 5k+ MCP · 90k+ yetenek                                                            |
| **Kaynak eşitleme**      | 🚫 Yok                 | 🚫 Yok               | 🚫 Yok               | ✅ Klasörlerin, depoların, akışların ve uygulamaların belleğe zamanlanmış eşitlenmesi              |
| **Orkestrasyon**         | ⚠️ Alt görevler        | ⚠️ Tek döngü         | ⚠️ Tek döngü         | 🚀 Ajan grafları + kontrol noktaları + uçtan uca şifreli A2A                                      |
| **İş akışları**          | 🚫 Yok                 | ⚠️ Betikler          | ⚠️ Betikler          | 🚀 Görsel, kalıcı, ajan tarafından önerilen, onay gerektiren                                      |
| **Toplantılar**          | 🚫 Yok                 | 🚫 Yok               | 🚫 Yok               | 🚀 Meet/Zoom/Teams/Webex'e katılır, konuşur, canlı transkript                                     |
| **Mesajlaşma kanalları** | 🚫 Yok                 | ⚠️ Birkaç tane       | ⚠️ Birkaç tane       | ✅ Yerel e-posta (IMAP/SMTP) dahil 15 kanal                                                       |
| **Yalnızca yerel mod**   | 🚫 Yalnızca bulut      | ⚠️ Kendi yerel modeliniz | ⚠️ Kendi yerel modeliniz | ✅ Tek anahtarla zorunlu kılınan Gizlilik Modu                                                |
| **Gözlemlenebilirlik**   | 🚫 Kapalı kutu         | ⚠️ Günlükler         | ⚠️ Günlükler         | ✅ Yeniden oynatılabilir çalıştırma kayıtları + çağrı başına maliyet hesabı                       |
| **API dağınıklığı**      | 🚫 Ek anahtarlar       | 🚫 BYOK              | 🚫 Çok sağlayıcılı   | ✅ Tek hesap                                                                                      |
| **Model yönlendirme**    | 🚫 Tek model           | ⚠️ Elle              | ⚠️ Elle              | ✅ Yerleşik                                                                                       |
| **Yerel araçlar**        | ✅ Yalnızca kod        | ✅ Yalnızca kod      | ✅ Yalnızca kod      | ✅ Kod + arama + kazıyıcı + tarayıcı + ses + medya üretimi                                        |

## Kaynaktan katkıda bulunma

Yeni katkıcı mısınız? Fork/PR iş akışı ve yerel doğrulama komutları için [`CONTRIBUTING.md`](../CONTRIBUTING.md) ile başlayın ya da [`CONTRIBUTING-BEGINNERS.md`](./CONTRIBUTING-BEGINNERS.md#optional--let-an-ai-coding-agent-guide-you) içindeki kopyala-yapıştır yapay zekâ ajanı istemini kullanın. Kısa yol şöyle:

1. Git, Node.js 24+, pnpm 10.10.0, Rust 1.96.1 (`rustfmt` + `clippy`), CMake, Ninja, ripgrep ve platformunuzun masaüstü derleme ön koşullarını kurun.
2. Depoyu fork edip klonlayın, ardından `vendor/` altındaki Rust bağımlılıklarının (tinyagents, tinyflows, tinychannels, tinymemory, motosan-ai-oauth, ...) çözümlenmesi için `pnpm install` öncesinde `git submodule update --init --recursive` komutunu çalıştırın.
3. Yalnızca web arayüzü çalışmaları için `pnpm dev`, masaüstü kabuğu için `pnpm --filter openhuman-app dev:app` (macOS) ya da `pnpm dev:app:win` (Windows) kullanın; PR açmadan önce `pnpm typecheck`, `pnpm format:check` ve `cargo check -p openhuman --lib` gibi odaklı kontrolleri çalıştırın.

`crates/` altındaki Rust çalışma alanı şu parçalara ayrılır: `crates/openhuman-core`
(`openhuman` paketi: çekirdek ve `openhuman-core` CLI), `crates/openhuman-app`
(ayrı bir Cargo dünyası olarak derlenen Tauri masaüstü kabuğu), `crates/openhuman-embed`
(çekirdeği gömmek için kütüphane arayüzü), `crates/openhuman-rpc` (ortak RPC
sözleşmeleri ve istemci) ve `crates/openhuman-tui` (terminal istemcisi). Tam yapı için
[Rust çekirdeğini derleme](../gitbooks/developing/building-rust-core.md) ve
[AGENTS.md](../AGENTS.md#repository-map) belgelerine bakın.

Daha ayrıntılı belgeler: [Mimari](https://tinyhumans.gitbook.io/openhuman/developing/architecture) · [Kurulum](https://tinyhumans.gitbook.io/openhuman/developing/getting-set-up) · [Bulut dağıtımı](../gitbooks/features/cloud-deploy.md).

# GitHub'da bize yıldız verin

_Projeyi takip etmek ve başkalarının da bulmasına yardımcı olmak için depoya yıldız verin._

<p align="center">
 <a href="https://www.star-history.com/#tinyhumansai/openhuman&type=date&legend=top-left">
 <picture>
 <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=tinyhumansai/openhuman&type=date&theme=dark&legend=top-left" />
 <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=tinyhumansai/openhuman&type=date&legend=top-left" />
 <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=tinyhumansai/openhuman&type=date&legend=top-left" />
 </picture>
 </a>
</p>

# Katkıcılar Onur Listesi

Biraz sevgi gösterin, siz de onur listesine girin. Katkıcılar ücretsiz hediye ürünler ve [Discord](https://guild.tinyhumans.ai/) sunucumuza özel erişim kazanır.

<a href="https://github.com/tinyhumansai/openhuman/graphs/contributors">
 <img src="https://contrib.rocks/image?repo=tinyhumansai/openhuman" alt="OpenHuman katkıcıları" />
</a>
