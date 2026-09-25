import type { HomeDict } from "../types";

/**
 * Vietnamese home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Xây dựng và tự động hóa với các mô hình bạn chọn",
  metaDescription:
    "Xây dựng phần mềm, làm việc với tệp và tự động hóa các tác vụ hằng ngày bằng tác tử mã nguồn mở cùng các mô hình AI chạy trên máy chủ hoặc cục bộ theo lựa chọn của bạn.",
  heroTitle: "Xây dựng và tự động hóa với các mô hình bạn chọn",
  heroIntro:
    "{brand} cung cấp các tác tử có thể xây dựng phần mềm, làm việc với tệp và biến những tác vụ lặp lại thành quy trình có thể tái sử dụng. Hãy cho chúng biết bạn muốn hoàn thành điều gì rồi chọn mô hình chạy trên máy chủ hoặc cục bộ phù hợp với công việc, đồng thời bạn có thể tự do chuyển đổi nhà cung cấp trong quá trình làm việc.",
  getCodewhale: "Tải Codewhale",
  heroInstallAria: "Lệnh cài đặt",
  exploreProduct: "Khám phá sản phẩm",
  shotPreview: "Xem trước terminal",
  shotBuild: "bản phát triển v{version}",
  screenshotAlt:
    "Bản phát triển Codewhale v{version}: dấu cá voi, phiên mới, ô nhập tin nhắn, quyền Ask, chế độ Work và trạng thái mô hình. Hiển thị từ đầu ra thực của một terminal biệt lập.",
  latestRelease: "Bản phát hành mới nhất {tag}",
  releaseUnavailable: "Không có trạng thái phát hành",
  currentSource: "Mã nguồn",
  sourceCandidate: "Chưa phát hành",
  providerRoutes: "{count} nhà cung cấp",
  publishedRelease: "đã phát hành",
  figcaptionSourceCandidate: "chưa phát hành",
  chapterTerminal: "Terminal của bạn",
  chapterTerminalTitle: "Bắt đầu với điều bạn muốn tạo ra",
  gainHeading:
    "Những việc bạn có thể làm với Codewhale",
  gainLede:
    "Hãy bắt đầu từ một dự án, một câu hỏi hoặc một tác vụ bạn muốn tự động hóa, rồi làm việc với một tác tử hoặc chia các phần của công việc lớn hơn cho nhiều tác tử.",
  gain: [
    [
      "Tạo ra điều bạn muốn",
      "Mô tả điều bạn muốn tạo ra và làm việc cùng các tác tử có thể đọc mã, chỉnh sửa tệp, chạy lệnh và kiểm tra kết quả."
    ],
    [
      "Tự động hóa công việc hằng ngày",
      "Tạo tập lệnh và quy trình cho các tác vụ bạn thường lặp lại để có thể chạy lại từ terminal bất cứ khi nào cần."
    ],
    [
      "Làm việc với nhiều mô hình",
      "Sử dụng mô hình chạy trên máy chủ hoặc cục bộ cho các tác tử, với những mô hình và vai trò khác nhau đảm nhận các phần công việc phù hợp."
    ]
  ],
  chapterModels: "Mô hình của bạn",
  modelsHeading: "Lựa chọn mô hình cho từng tác vụ",
  modelsBody:
    "Bạn có thể kết nối trực tiếp với một nhà cung cấp mô hình, dùng cổng kết nối để truy cập nhiều nhà cung cấp hoặc chạy mô hình cục bộ, rồi chọn mô hình cho từng phiên khi làm việc.",
  modelsFacts: [
    ["Hosted", "Khóa API của bạn, lưu bằng codewhale auth set --provider <id>"],
    ["Gateway", "Một endpoint cho nhiều mô hình, nhà cung cấp vẫn do bạn chọn"],
    ["Cục bộ", "vLLM, SGLang, Ollama trên localhost — thường không cần khóa"],
  ],
  modelsLink: "Khám phá mô hình và nhà cung cấp",
  startHeading: "Bắt đầu sử dụng Codewhale",
  startLede:
    "Sau khi cài đặt Codewhale và kết nối một mô hình, bạn có thể mô tả tác vụ đầu tiên trong terminal và thêm Fleet khi muốn nhiều tác tử cùng chia sẻ công việc.",
  startGuideLink: "Đọc hướng dẫn bắt đầu",
  startVocabularyLink: "Xem thuật ngữ sản phẩm",
  chapterAccount: "Tải Codewhale",
  availabilityHeading: "Nơi bạn có thể sử dụng Codewhale",
  availabilityLede:
    "Bạn có thể sử dụng Codewhale trong terminal ngay hôm nay, trong khi chúng tôi đang phát triển ứng dụng web, ứng dụng máy tính và máy tính đám mây.",
  availability: [
    [
      "Terminal",
      "Đã phát hành",
      "Các bản nhị phân phát hành trên GitHub dành cho Linux, macOS và Windows; bạn cũng có thể cài qua npm hoặc Cargo. Phiên bản Android trên Termux là bản xem trước."
    ],
    [
      "Ứng dụng web",
      "Bản xem trước đang phát triển",
      "Truy cập tài khoản và ghép nối trình duyệt trong bản xem trước đang phát triển."
    ],
    [
      "Máy tính để bàn",
      "Bản phát triển",
      "Ứng dụng macOS đang được phát triển; bản tải xuống công khai sẽ có sau."
    ],
    [
      "Máy tính đám mây",
      "Đang phát triển",
      "Máy tính do nhà cung cấp vận hành để chạy tác vụ của bạn."
    ]
  ],
  availabilityNote:
    "Bạn có thể dùng terminal mà không cần tài khoản Codewhale; phí sử dụng các mô hình do nhà cung cấp vận hành sẽ do nhà cung cấp đó tính.",
  accountLink: "Tạo tài khoản",
  surfacesHeading: "Các cách làm việc với Codewhale",
  surfaces: [
    ["TUI", "Làm việc tương tác trong terminal"],
    ["codewhale exec", "Script và CI"],
    ["Trình khách web cục bộ","Giao diện localhost; không gian làm việc trên trình duyệt do máy chủ cung cấp vẫn đang được phát triển"],
    ["Runtime API + MCP", "Tích hợp cục bộ"],
    ["Fleet","Nhiều tác tử cùng làm một việc"],
  ],
  runtimeLink: "Khám phá các tích hợp",
  installBandHeading: "Cài đặt Codewhale trên macOS hoặc Linux",
  copy: "Sao chép",
  copied: "Đã sao chép ✓",
  binaries: "Bản nhị phân",
  chinaMirrors: "Mirror Trung Quốc",
  installGuideLink: "Đọc hướng dẫn cài đặt",
  communityHeading: "Cùng giúp Codewhale tốt hơn",
  communityBody:
    "Dù bạn phát hiện lỗi, có ý tưởng về một tính năng hay muốn gửi pull request đầu tiên, chúng tôi đều muốn lắng nghe và cùng bạn thực hiện những bước tiếp theo.",
  communityLinksAria: "Liên kết cộng đồng",
  contribute: "Gửi pull request",
};
