<!-- source: README.md sha256:29c349b6f2b4 -->
# Codewhale

Codewhale एक ओपन सोर्स एजेंट है जो आपकी पसंद के होस्ट किए गए या लोकल मॉडल से आपका प्रोजेक्ट पढ़ता है, फ़ाइलें संपादित करता है, कमांड चलाता है और अपने काम की जाँच करता है। टर्मिनल में एक काम से शुरुआत करें। बड़े काम के हिस्से अलग-अलग मॉडल और भूमिकाओं वाले एजेंटों को सौंपें।

![टर्मिनल में चलता Codewhale](web/public/codewhale-tui-d7a9a1c.png)

*v0.10.0 के विकासाधीन बिल्ड से टर्मिनल का पूर्वावलोकन।*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## इंस्टॉल करें

macOS या Linux पर नए इंस्टॉलेशन के लिए आधिकारिक GitHub रिलीज़ इस्तेमाल करें:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

इंस्टॉलर सबसे नई प्रकाशित रिलीज़ चुनता है। [बदलावों की सूची](CHANGELOG.md) में अगली रिलीज़ के अभी तक अप्रकाशित कैंडिडेट का भी विवरण है; रिलीज़ उपलब्ध होने तक ये बदलाव प्रकाशित डाउनलोड में शामिल नहीं होते।

Windows पर [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest) से उपयुक्त इंस्टॉलर या आर्काइव डाउनलोड करें। मौजूदा सीधे इंस्टॉलेशन को अपडेट करने के लिए `codewhale update` चलाएँ; केवल जाँच के लिए `codewhale update --check` इस्तेमाल करें। अपडेटर executable का पथ दिखाता है और नए बिल्ड सुरक्षित रखता है। npm और Cargo वैकल्पिक पैकेजिंग तरीके हैं। पैकेज मैनेजर वाले इंस्टॉलेशन से माइग्रेशन और PATH के लिए [इंस्टॉलेशन गाइड](docs/INSTALL.md) देखें।

पहली बार चलाने पर Codewhale आपको किसी प्रोवाइडर से जुड़ने या Codewhale को ऑफ़लाइन कॉन्फ़िगर करने में मदद करता है। मॉडल से जवाब पाने के लिए किसी होस्ट किए गए या लोकल मॉडल से कनेक्शन ज़रूरी है। Codewhale अतिरिक्त पैकेजिंग विकल्पों के रूप में npm और Cargo के साथ-साथ Docker, Nix, Scoop, Android/Termux और वैकल्पिक CNB मिरर का भी समर्थन करता है। पैकेज मैनेजर से प्रबंधित मौजूदा इंस्टॉलेशन के लिए माइग्रेशन के निर्देश मिलते हैं। [इंस्टॉलेशन और PATH से जुड़ी मदद](docs/INSTALL.md) देखें।

हर शेल में Tab completion के लिए केवल एक कमांड चाहिए — `codewhale completion bash|zsh|fish|powershell|elvish`। [शेल कंप्लीशन](docs/INSTALL.md#8-shell-completions) देखें।

## उपयोग

अपने प्रोजेक्ट फ़ोल्डर में टर्मिनल खोलें और `codewhale` चलाएँ। `/provider` से अपना प्रोवाइडर और `/model` से अपना मॉडल चुनें। फिर कोई ठोस काम बताएँ:

```text
Fix the failing tests and explain what changed.
```

या TUI खोले बिना कोई कार्य चलाएँ:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale आपकी रिपॉज़िटरी पढ़ सकता है, फ़ाइलें संपादित कर सकता है, कमांड चला सकता है, परिणामों की जाँच कर सकता है और लक्ष्य की ओर काम जारी रख सकता है। फ़ाइलें बदले या शेल कमांड चलाए बिना पड़ताल करने के लिए `/mode plan` इस्तेमाल करें, और बदलाव करवाने के लिए `/mode work` चुनें। Ask, Auto-Review या Full Access चुनने के लिए `Shift+Tab` दबाएँ; [मोड और अनुमतियों की गाइड](docs/MODES.md) बताती है कि हर विकल्प में क्या करने की अनुमति है।

## टर्मिनल, ऐप और Computer Use

टर्मिनल और ग्राफ़िकल क्लाइंट Codewhale Runtime से जुड़ते हैं, जो एजेंट और उसके टूल चलाता है:

- **टर्मिनल:** `codewhale` इंटरैक्टिव इंटरफ़ेस खोलता है; `codewhale exec` किसी स्क्रिप्ट या CI जॉब से काम चलाता है।
- **लोकल ब्राउज़र:** `codewhale web` उसी रनटाइम के लिए पैकेज में शामिल [लोकल वेब क्लाइंट](docs/WEB.md) खोलता है।
- **Codewhale डेस्कटॉप ऐप (GPUI):** नेटिव GPUI डेस्कटॉप ऐप प्रोडक्ट-क्लाइंट दिशा है (2026-09-14 निर्णय; फेज़ मैप निजी codehwhale-gpui रेपो के docs/TRANSITION.md में है)। app.codewhale.net पर होस्ट किया गया वेब ऐप चरणों में समाप्त होगा; मार्केटिंग साइट, साइन-इन, बिलिंग, कानूनी और डाउनलोड पेज वेब पर स्थायी रूप से बने रहेंगे। उनकी उपलब्धता [प्रोडक्ट पेज](https://codewhale.net/en/product) पर दी गई है।

**Computer Use दूसरे ऐप देखने और उनके साथ इंटरैक्ट करने के लिए टूल जोड़ता है।** प्लगइन मौजूदा सोर्स कोड में शामिल है। इस्तेमाल से पहले उसके माँगे गए एक्सेस की समीक्षा करें और उसे सक्षम करें; OS की अनुमतियाँ और प्लेटफ़ॉर्म की आवश्यकताएँ तब भी लागू होती हैं। शामिल [Computer Use गाइड](crates/tui/plugins/computer-use/README.md) और [प्लगइन सेटअप](docs/PLUGINS.md) देखें।

समुदाय द्वारा अनुरक्षित VS Code का CodeWhale एक्सटेंशन साइडबार से लोकल Runtime से जुड़ता है। इसे [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode) से इंस्टॉल करें; सोर्स कोड [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode) पर है।

## Codewhale क्यों

- **अपने मॉडल चुनें।** होस्ट किए गए प्रोवाइडर या Ollama, vLLM अथवा SGLang के माध्यम से लोकल मॉडल जोड़ें। प्रोवाइडर बदलने के लिए `/provider` और मॉडल चुनने के लिए `/model` इस्तेमाल करें।
- **नियंत्रण अपने पास रखें।** प्रस्तावित कार्रवाइयों और उनसे फ़ाइलों में हुए बदलावों की जाँच करें। अनुमोदन की सेटिंग तय करती हैं कि समीक्षा कब ज़रूरी है; Full Access भी नीति की बाध्यकारी सीमाओं का पालन करता है। `/undo` और `/restore` बदलावों के बाद वर्कस्पेस बहाल करने में मदद करते हैं।
- **लंबे काम को व्यवस्थित रखें।** सेशन सहेजें, स्थायी `/goal` तय करें, वर्कफ़्लो चलने से पहले उनकी समीक्षा करें और एजेंटों के आंतरिक निर्देशों को अपनी बातचीत में जोड़े बिना उनका समन्वय करें।
- **अपने मौजूदा एजेंट को विस्तृत करें।** MCP सर्वर और स्किल जोड़ें, हुक कॉन्फ़िगर करें और एजेंट की भूमिकाओं को अपने प्रोजेक्ट या निजी सेटिंग में पढ़ने योग्य फ़ाइलों के रूप में रखें।

कमांड और कीबोर्ड शॉर्टकट देखने के लिए TUI में `/help` चलाएँ।

## सुरक्षा

Codewhale आपकी मशीन पर उतने ही एक्सेस के साथ चलता है जितना आप उसे देते हैं। अनुमोदन मोड और रिपॉज़िटरी के नियम एजेंट की गतिविधियों को सीमित करते हैं; समर्थित सिस्टम पर वैकल्पिक OS सैंडबॉक्सिंग अधिक मज़बूत निष्पादन सीमा जोड़ती है। जिन मॉडलों की कीमत ज्ञात नहीं है, उन्हें मुफ़्त बताने के बजाय अज्ञात ही दिखाया जाता है।

नीतियों का सटीक क्रम जानने के लिए [अधिकार क्रम](docs/AUTHORIZATION_ORDER.md) और लोकल सेटिंग के लिए [कॉन्फ़िगरेशन](docs/CONFIGURATION.md) पढ़ें।

## दस्तावेज़

- [प्रोवाइडर और लोकल मॉडल](docs/PROVIDERS.md)
- [एजेंट टीमें](docs/FLEET.md)
- [MCP](docs/MCP.md), [हुक](docs/HOOKS.md) और [कॉन्फ़िगरेशन](docs/CONFIGURATION.md)
- [लोकल वेब क्लाइंट](docs/WEB.md)
- [सभी दस्तावेज़](docs)
- [रिपॉज़िटरी की संरचना और योगदान गाइड](CONTRIBUTING.md#project-structure)

## समुदाय से जुड़ें

**बग रिपोर्ट, नए फ़ीचर के सुझाव और pull request का स्वागत है**, चाहे आप Codewhale का कई महीनों से इस्तेमाल कर रहे हों या पहली बार आज़मा रहे हों। यदि कोई प्रोवाइडर उपलब्ध नहीं है, कोई वर्कफ़्लो असहज है या टर्मिनल UI आपके काम में बाधा डालता है, तो [issue खोलें](https://github.com/Hmbown/CodeWhale/issues/new/choose) या [pull request भेजें](CONTRIBUTING.md), ताकि हम मिलकर इसे बेहतर बना सकें। पहले योगदान का स्वागत है और स्वीकार किए गए काम का श्रेय योगदानकर्ताओं के पास रहता है।

[Discord](https://discord.gg/37gfS3ksug) से जुड़ें, या WeChat पर Hunter (`hunterbown`) को जोड़कर Whale Brothers समूह में शामिल होने के लिए कहें।

## प्रोजेक्ट का इतिहास

Codewhale की शुरुआत `deepseek-tui` के रूप में हुई थी और यह आज भी उसके कॉन्फ़िगरेशन तथा सेशन के साथ संगतता बनाए रखता है। अब यह किसी प्रोवाइडर पर निर्भर नहीं है, स्वतंत्र रूप से अनुरक्षित है और किसी भी मॉडल प्रोवाइडर से संबद्ध नहीं है।

हर योगदानकर्ता और प्रोजेक्ट को आगे बढ़ाने वाले ओपन सोर्स समुदायों का धन्यवाद। [योगदानकर्ताओं का रिकॉर्ड](docs/CONTRIBUTORS.md) देखें।

## लाइसेंस

[MIT](LICENSE)। अन्य ओपन सोर्स प्रोजेक्ट से लिए और अनुकूलित किए गए हिस्से [थर्ड-पार्टी नोटिस](docs/THIRD_PARTY_NOTICES.md) में दर्ज हैं।
