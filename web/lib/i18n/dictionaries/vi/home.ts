import type { HomeDict } from "../types";

/**
 * Vietnamese home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: tác tử lập trình mã nguồn mở cho mọi mô hình",
  metaDescription:
    "Codewhale là tác tử lập trình mã nguồn mở dành cho terminal. Codewhale đọc dự án, chỉnh sửa tệp và chạy kiểm thử bằng mô hình chạy trên máy chủ hoặc cục bộ mà bạn chọn.",
  heroTitle: "Tác tử lập trình mã nguồn mở cho mọi mô hình",
  heroIntro:
    "{brand} đọc dự án, chỉnh sửa tệp và chạy kiểm thử của bạn ngay từ terminal. Hãy kết nối một mô hình chạy trên máy chủ hoặc cục bộ, rồi chọn những thao tác cần bạn phê duyệt.",
  getCodewhale: "Cài đặt Codewhale",
  heroInstallAria: "Lệnh cài đặt",
  exploreProduct: "Xem cách hoạt động",
  shotPreview: "Xem trước terminal",
  shotBuild: "bản phát triển v{version}",
  screenshotAlt:
    "Bản phát triển Codewhale v{version}: dấu cá voi, phiên mới, ô nhập tin nhắn, quyền Ask, chế độ Work và trạng thái mô hình. Hiển thị từ đầu ra thực của một terminal biệt lập.",
  latestRelease: "Bản phát hành mới nhất {tag}",
  releaseUnavailable: "Không có trạng thái phát hành",
  currentSource: "Mã nguồn",
  sourceCandidate: "Chưa phát hành",
  publishedRelease: "đã phát hành",
  figcaptionSourceCandidate: "chưa phát hành",
  chapterTerminal: "Terminal của bạn",
  chapterTerminalTitle: "Theo dõi từng chỉnh sửa và lệnh khi chạy",
  gainHeading: "Giao tác vụ và vẫn giữ quyền kiểm soát",
  gainLede:
    "Hãy yêu cầu một kết quả: sửa lỗi, giải thích một module hoặc tự động hóa một tác vụ bạn thường lặp lại. Bắt đầu với một tác tử, rồi thêm tác tử khi công việc lớn hơn.",
  gain: [
    [
      "Thay đổi mã và kiểm tra",
      "Tác tử xem xét dự án, chỉnh sửa tệp và chạy kiểm thử của bạn. Bạn theo dõi từng chỉnh sửa và kết quả lệnh trong khi tác tử làm việc."
    ],
    [
      "Tự động hóa công việc lặp lại",
      "Chạy codewhale exec từ script và CI. Dùng Fleet để chia một công việc lớn cho nhiều tác tử."
    ],
    [
      "Giữ quyền kiểm soát",
      "Đặt quyền trước khi bắt đầu, trả lời các yêu cầu phê duyệt và dừng tác vụ bất cứ lúc nào. Chạy /receipts để liệt kê mọi tệp, lệnh và phê duyệt trong một phiên."
    ]
  ],
  chapterModels: "Mô hình của bạn",
  modelsHeading: "Chọn mô hình cho từng tác vụ",
  modelsBody:
    "Với mỗi phiên, hãy chọn một nhà cung cấp tích hợp sẵn, một endpoint tương thích OpenAI bất kỳ hoặc một mô hình cục bộ. Kết nối mô hình của bạn luôn tách biệt với tài khoản Codewhale.",
  modelsFacts: [
    ["Hosted", "Khóa API của bạn, lưu bằng codewhale auth set --provider <id>"],
    ["Gateway", "Một endpoint cho nhiều mô hình; bạn vẫn chọn nhà cung cấp"],
    ["Cục bộ", "vLLM, SGLang hoặc Ollama trên localhost, thường không cần khóa"],
  ],
  modelsLink: "Xem mô hình và nhà cung cấp",
  startHeading: "Cài đặt, kết nối mô hình, chạy tác vụ",
  startLede:
    "Chạy tác vụ đầu tiên trong ba bước từ thư mục dự án của bạn. Hãy thêm Fleet sau nếu công việc cần nhiều tác tử.",
  startGuideLink: "Làm theo hướng dẫn bắt đầu",
  startVocabularyLink: "Xem thuật ngữ sản phẩm",
  chapterAvailability: "Chạy ở đâu",
  availabilityHeading: "Dùng ngay trong terminal của bạn",
  availabilityLede:
    "Bạn có thể dùng terminal, client trình duyệt cục bộ hoặc CodeWhale GUI của cộng đồng ngay bây giờ. Ứng dụng desktop và ứng dụng web lưu trữ trực tuyến đang được xây dựng lại đều đang phát triển và dùng chung một mô hình phiên.",
  availability: [
    [
      "Terminal và trình duyệt cục bộ",
      "Đã phát hành",
      "Cài đặt trên Linux, macOS hoặc Windows, rồi chạy codewhale, hoặc codewhale web để dùng client trình duyệt cục bộ. Bạn cũng có thể dùng npm và Cargo; bản Android trên Termux là bản xem trước."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Có sẵn",
      "Một dự án riêng do cộng đồng duy trì: trò chuyện, chủ đề và thay đổi tệp trong thanh bên VS Code trên cùng Codewhale Runtime. Cài đặt từ VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Ứng dụng web lưu trữ trực tuyến",
      "Bản xem trước đang phát triển",
      "Đang được xây dựng lại cho khớp với ứng dụng desktop. Hiện nay bạn có thể đăng nhập, rồi gõ /rc trong một phiên terminal đang chạy để tiếp tục trên web; việc thực thi tác vụ lưu trữ trực tuyến vẫn đang được thẩm định."
    ],
    [
      "Máy tính để bàn",
      "Bản phát triển",
      "Ứng dụng gốc đang trở thành client chính của Codewhale: thư mục, cuộc trò chuyện và kết nối mô hình trong một cửa sổ. Hiện chưa có bản tải xuống công khai."
    ],
    [
      "Máy tính đám mây",
      "Đang phát triển",
      "Máy tính lưu trữ trực tuyến chạy tác vụ của bạn."
    ]
  ],
  availabilityNote:
    "Terminal, trình duyệt cục bộ và GUI không cần tài khoản Codewhale. Web lưu trữ trực tuyến và desktop dùng tài khoản, nhưng tài khoản không thay thế kết nối mô hình của bạn; nhà cung cấp tính phí mức sử dụng trên khóa của riêng bạn.",
  accountLink: "Tạo tài khoản",
  surfacesHeading: "Mở rộng phạm vi mà tác tử có thể tiếp cận",
  surfaces: [
    ["Tệp và lệnh", "Đọc dự án, chỉnh sửa tệp, chạy kiểm thử và xem đầu ra trong phạm vi quyền bạn đặt."],
    ["Plugin và MCP", "Kết nối thêm công cụ và dịch vụ. Mỗi plugin luôn tắt cho đến khi bạn xem xét và bật plugin đó."],
    ["Computer Use · xem trước", "Một plugin cho phép tác tử xem và điều khiển các ứng dụng khác. Bạn tự bật plugin này và cấp các quyền hệ thống mà plugin yêu cầu."],
    ["Phiên đã lưu", "Giữ cuộc trò chuyện và kết quả công cụ cùng nhau, rồi tiếp tục thay vì bắt đầu lại. Trình duyệt cục bộ mở cùng phiên đó trên máy tính của bạn."],
    ["Fleet", "Giao các phần của một tác vụ cho các tác tử có mô hình và vai trò khác nhau, rồi theo dõi tiến độ của các tác tử đó."],
  ],
  runtimeLink: "Xem tất cả tích hợp",
  installBandHeading: "Cài đặt trên macOS hoặc Linux",
  copy: "Sao chép",
  copied: "Đã sao chép ✓",
  binaries: "Bản nhị phân",
  chinaMirrors: "Mirror Trung Quốc",
  installGuideLink: "Đọc hướng dẫn cài đặt",
  communityHeading: "Cùng chúng tôi xây dựng Codewhale",
  communityBody:
    "Báo lỗi, đề xuất tính năng hoặc gửi pull request đầu tiên của bạn trên GitHub. Chúng tôi hoan nghênh các bản sửa nhỏ đã được kiểm thử.",
  communityLinksAria: "Liên kết cộng đồng",
  contribute: "Gửi pull request",
};
