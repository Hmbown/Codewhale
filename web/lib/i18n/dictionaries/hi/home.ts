import type { HomeDict } from "../types";

/**
 * Hindi home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — अपनी पसंद के मॉडल से बनाएँ और काम स्वचालित करें",
  metaDescription:
    "ओपन-सोर्स एजेंट और अपनी पसंद के होस्टेड या लोकल AI मॉडल से सॉफ़्टवेयर बनाएँ, अपनी फ़ाइलों पर काम करें और रोज़मर्रा के काम स्वचालित करें।",
  heroTitle: "अपनी पसंद के मॉडल से बनाएँ और काम स्वचालित करें",
  heroIntro:
    "{brand} आपको ऐसे एजेंट देता है जो सॉफ़्टवेयर बना सकते हैं, आपकी फ़ाइलों पर काम कर सकते हैं और दोहराए जाने वाले कामों को बार-बार इस्तेमाल होने वाले वर्कफ़्लो में बदल सकते हैं। उन्हें बताएँ कि आप क्या करना चाहते हैं और काम के लिए उपयुक्त होस्टेड या लोकल मॉडल चुनें, जिन्हें इस्तेमाल करते हुए आप अपनी ज़रूरत के अनुसार प्रदाता भी बदल सकते हैं।",
  getCodewhale: "Codewhale लें",
  exploreProduct: "उत्पाद देखें",
  shotPreview: "टर्मिनल पूर्वावलोकन",
  shotBuild: "v{version} डेवलपमेंट बिल्ड",
  screenshotAlt:
    "Codewhale v{version} डेवलपमेंट बिल्ड: व्हेल चिह्न, नया सत्र, संदेश इनपुट, Ask अनुमतियाँ, Work मोड और मॉडल की स्थिति। अलग टर्मिनल के वास्तविक आउटपुट से बनाया गया दृश्य।",
  latestRelease: "नवीनतम रिलीज़ {tag}",
  releaseUnavailable: "रिलीज़ स्थिति उपलब्ध नहीं",
  currentSource: "सोर्स",
  sourceCandidate: "अप्रकाशित",
  providerRoutes: "{count} प्रोवाइडर",
  publishedRelease: "प्रकाशित",
  figcaptionSourceCandidate: "अप्रकाशित",
  chapterTerminal: "आपका टर्मिनल",
  chapterTerminalTitle: "किसी ऐसी चीज़ से शुरू करें जिसे आप बनाना चाहते हैं",
  gainHeading:
    "Codewhale से आप क्या कर सकते हैं",
  gainLede:
    "किसी प्रोजेक्ट, सवाल या ऐसे काम से शुरू करें जिसे आप स्वचालित करना चाहते हैं, फिर एक एजेंट के साथ काम करें या बड़े काम के अलग-अलग हिस्से कई एजेंटों को सौंपें।",
  gain: [
    [
      "कुछ बनाएँ",
      "बताएँ कि आप क्या बनाना चाहते हैं और ऐसे एजेंटों के साथ काम करें जो आपका कोड पढ़ सकते हैं, फ़ाइलें बदल सकते हैं, कमांड चला सकते हैं और नतीजा जाँच सकते हैं।"
    ],
    [
      "रोज़मर्रा के काम स्वचालित करें",
      "दोहराए जाने वाले कामों के लिए स्क्रिप्ट और वर्कफ़्लो बनाएँ, ताकि ज़रूरत पड़ने पर उन्हें टर्मिनल से फिर चला सकें।"
    ],
    [
      "अलग-अलग मॉडल के साथ काम करें",
      "अपने एजेंटों के लिए होस्टेड या लोकल मॉडल इस्तेमाल करें और काम के हिस्से उनके लिए उपयुक्त मॉडल और भूमिकाओं को सौंपें।"
    ]
  ],
  chapterModels: "आपके मॉडल",
  modelsHeading: "हर काम के लिए मॉडल का चुनाव",
  modelsBody:
    "होस्टेड मॉडल देने वाले किसी प्रदाता से सीधे जुड़ें, कई प्रदाताओं से जुड़ने के लिए गेटवे इस्तेमाल करें या मॉडल लोकल चलाएँ, फिर काम करते हुए चुनें कि हर सेशन में कौन-सा मॉडल इस्तेमाल हो।",
  modelsFacts: [
    ["होस्टेड", "आपकी अपनी API कुंजी, codewhale auth set --provider <id> से सेव"],
    ["गेटवे", "कई मॉडलों के लिए एक एंडपॉइंट, प्रोवाइडर फिर भी आप चुनते हैं"],
    ["लोकल", "localhost पर vLLM, SGLang, Ollama — आमतौर पर बिना कुंजी"],
  ],
  modelsLink: "मॉडल और प्रदाता देखें",
  startHeading: "Codewhale के साथ शुरुआत करें",
  startLede:
    "Codewhale इंस्टॉल करके मॉडल कनेक्ट करने के बाद आप टर्मिनल में अपना पहला काम बता सकते हैं और जब कई एजेंटों के बीच काम बाँटना चाहें, तब Fleet जोड़ सकते हैं।",
  startGuideLink: "शुरुआती गाइड पढ़ें",
  startVocabularyLink: "उत्पाद शब्दावली देखें",
  chapterAccount: "Codewhale लें",
  availabilityHeading: "आप Codewhale कहाँ इस्तेमाल कर सकते हैं",
  availabilityLede:
    "आप आज ही Codewhale को अपने टर्मिनल में इस्तेमाल कर सकते हैं, जबकि हम वेब ऐप, डेस्कटॉप ऐप और क्लाउड कंप्यूटर बनाने पर काम कर रहे हैं।",
  availability: [
    [
      "टर्मिनल",
      "जारी",
      "Linux, macOS और Windows के लिए GitHub रिलीज़ बाइनरी उपलब्ध हैं; npm और Cargo वैकल्पिक तरीके हैं। Termux पर Android अभी प्रीव्यू में है।"
    ],
    [
      "वेब ऐप",
      "डेवलपमेंट प्रीव्यू",
      "डेवलपमेंट प्रीव्यू में खाते तक पहुँच और ब्राउज़र पेयरिंग।"
    ],
    [
      "डेस्कटॉप",
      "विकासाधीन बिल्ड",
      "macOS ऐप अभी विकासाधीन है; सभी के लिए डाउनलोड बाद में उपलब्ध होगा।"
    ],
    [
      "क्लाउड कंप्यूटर",
      "विकासाधीन",
      "आपके काम चलाने के लिए होस्टेड कंप्यूटर।"
    ]
  ],
  availabilityNote:
    "आप Codewhale खाते के बिना टर्मिनल इस्तेमाल कर सकते हैं और होस्टेड मॉडल के इस्तेमाल का शुल्क आपका प्रदाता लेता है।",
  accountLink: "खाता बनाएँ",
  surfacesHeading: "Codewhale के साथ काम करने के तरीके",
  surfaces: [
    ["TUI", "टर्मिनल में इंटरैक्टिव काम"],
    ["codewhale exec", "स्क्रिप्ट और CI"],
    ["लोकल वेब क्लाइंट","localhost पर इंटरफ़ेस; ब्राउज़र में होस्टेड कार्यक्षेत्र विकासाधीन है"],
    ["Runtime API + MCP", "लोकल इंटीग्रेशन"],
    ["Fleet","एक काम पर कई एजेंट"],
  ],
  runtimeLink: "इंटीग्रेशन देखें",
  installBandHeading: "macOS या Linux पर Codewhale इंस्टॉल करें",
  copy: "कॉपी करें",
  copied: "कॉपी हो गया ✓",
  binaries: "बाइनरी",
  chinaMirrors: "चीन मिरर",
  installGuideLink: "इंस्टॉल गाइड पढ़ें",
  communityHeading: "Codewhale को बेहतर बनाने में मदद करें",
  communityBody:
    "चाहे आपको कोई बग मिला हो, किसी सुविधा का विचार आया हो या आप अपना पहला pull request भेजना चाहते हों, हम आपकी बात सुनना और आगे का काम मिलकर करना चाहेंगे।",
  communityLinksAria: "समुदाय लिंक",
  contribute: "Pull request भेजें",
};
