import type { HomeDict } from "../types";

/** Vietnamese home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: xây dựng với mô hình và công cụ của bạn",
  metaDescription:
    "Tạo ứng dụng, tự động hóa quy trình và làm việc với công cụ đã kết nối bằng Codewhale. Mã nguồn mở, dùng API mô hình của bạn hoặc suy luận cục bộ và tự lưu trữ.",
  heroTitle: "Biến điều bạn nghĩ thành sản phẩm.",
  heroIntro:
    "Tạo ứng dụng, tự động hóa quy trình hoặc biến tài liệu nghiên cứu thành kết quả hữu ích. {brand} làm việc với API mô hình bạn đang dùng, hạ tầng suy luận riêng và các công cụ bạn kết nối.",
  getCodewhale: "Cài đặt Codewhale",
  heroInstallAria: "Lệnh cài đặt",
  exploreProduct: "Khám phá Codewhale",
  shotPreview: "Xem trước terminal",
  screenshotAlt:
    "Codewhale v{version}: ảnh chụp terminal với cuộc trò chuyện, ô nhập tin nhắn và điều khiển phiên.",
  latestRelease: "Bản phát hành mới nhất {tag}",
  releaseUnavailable: "Không có trạng thái phát hành",
  currentSource: "Mã nguồn",
  sourceCandidate: "Chưa phát hành",
  publishedRelease: "đã phát hành",
  gainHeading: "Tạo ra điều hữu ích.",
  gainLede:
    "Bắt đầu từ kết quả bạn muốn. Codewhale làm việc với tệp, lệnh và công cụ đã kết nối; bạn quyết định quyền truy cập và phê duyệt.",
  gain: [
    [
      "Tạo ứng dụng và công cụ",
      "Biến ý tưởng thành ứng dụng chạy được, script hữu ích hoặc tính năng trong dự án có sẵn. Để tác tử cùng bạn viết, chạy và kiểm thử."
    ],
    [
      "Tự động hóa việc lặp lại",
      "Biến tác vụ định kỳ thành quy trình chạy từ terminal, script hoặc CI. Dùng đội tác tử Fleet khi công việc có thể chạy song song."
    ],
    [
      "Kết nối công cụ bạn dùng",
      "Kết nối công cụ như Gmail và Slack qua plugin, máy chủ MCP hoặc API. Làm việc với các dịch vụ đó cùng tệp và lệnh của bạn."
    ]
  ],
  exampleTasks: [
    "Tạo một ứng dụng đặt lịch hẹn.",
    "Biến CSV bán hàng thành báo cáo hằng tuần có thể tạo lại.",
    "Biến thư trong email đã kết nối của tôi thành danh sách việc cần làm.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "Báo cáo doanh số hằng tuần",
  reportSampleLabel: "Báo cáo minh họa · dữ liệu mẫu",
  reportDescription: "Yêu cầu Codewhale nhóm đơn hàng theo tuần và lưu quy trình để dùng với tệp CSV tiếp theo.",
  reportSourceLabel: "Dữ liệu đầu vào:",
  reportColumns: ["Đầu tuần","Đơn hàng","Doanh số (USD)"],
  reportTotalLabel: "Tổng",
  reportTrend: "Thay đổi doanh số từ tuần đầu đến tuần cuối: {change}.",
  reportDownloadLabel: "Tải báo cáo CSV",
  chapterModels: "Mô hình của bạn",
  modelsHeading: "Tiếp tục dùng mô hình bạn chọn.",
  modelsBody:
    "Kết nối API mô hình bạn đang trả phí, dùng gateway tương thích hoặc chạy suy luận trên phần cứng riêng. Chọn mô hình cho từng phiên và từng tác tử trong Fleet.",
  modelsFacts: [
    [
      "Tài khoản API của bạn",
      "Kết nối OpenAI, Anthropic, Google hoặc DeepSeek bằng khóa riêng."
    ],
    [
      "Gateway của bạn",
      "Dùng endpoint tương thích OpenAI và chọn mô hình nó cung cấp."
    ],
    [
      "Suy luận của bạn",
      "Chạy mô hình cục bộ hoặc tự lưu trữ bằng Ollama, vLLM hoặc SGLang."
    ]
  ],
  modelsLink: "Xem mô hình và nhà cung cấp",
  startHeading: "Mang theo tác vụ. Bắt đầu thôi.",
  startLede:
    "Cài Codewhale, kết nối mô hình và giao một việc đáng làm. Bắt đầu với một tác tử; thêm công cụ hoặc đội khi cần.",
  startGuideLink: "Làm theo hướng dẫn bắt đầu",
  startVocabularyLink: "Xem thuật ngữ sản phẩm",
  chapterAvailability: "Chạy ở đâu",
  availabilityHeading: "Bắt đầu trong terminal.",
  availabilityLede:
    "Terminal và client trình duyệt cục bộ đã có sẵn. Ứng dụng desktop gốc và ứng dụng web lưu trữ trực tuyến đang được phát triển.",
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
    "Terminal, trình duyệt cục bộ và GUI không cần tài khoản Codewhale. Web lưu trữ trực tuyến và ứng dụng desktop dùng tài khoản. Khi bạn dùng khóa riêng của mình từ nhà cung cấp, nhà cung cấp đó tính phí phần sử dụng này.",
  accountLink: "Tạo tài khoản",
  surfacesHeading: "Một tác vụ. Tệp, ứng dụng và tác tử của bạn.",
  surfaces: [
    [
      "Tệp và terminal",
      "Tạo tệp, chạy lệnh, xem dữ liệu và kiểm thử sản phẩm. Bạn đặt thư mục làm việc và quyền truy cập."
    ],
    [
      "Plugin và ứng dụng đã kết nối",
      "Thêm kỹ năng và công cụ qua plugin và MCP. Xem xét và bật các kết nối bạn muốn tác tử sử dụng."
    ],
    [
      "Trình duyệt và máy tính · xem trước",
      "Dùng công cụ trình duyệt và plugin Computer Use để làm việc trong ứng dụng và website với quyền bạn cấp."
    ],
    [
      "Phiên có thể tiếp tục",
      "Giữ cuộc trò chuyện, kết quả công cụ và lịch sử công việc cùng nhau. Tiếp tục tác vụ trong terminal hoặc client trình duyệt cục bộ."
    ],
    [
      "Đội tác tử",
      "Dùng Fleet chia công việc lớn cho tác tử có vai trò và mô hình khác nhau, rồi theo dõi tiến độ ở một nơi."
    ]
  ],
  runtimeLink: "Xem tất cả tích hợp",
  installBandHeading: "Cài đặt trên macOS hoặc Linux",
  copy: "Sao chép",
  copied: "Đã sao chép ✓",
  binaries: "Bản nhị phân",
  chinaMirrors: "Mirror Trung Quốc",
  installGuideLink: "Đọc hướng dẫn cài đặt",
  communityHeading: "Biến Codewhale thành công cụ của bạn.",
  communityBody:
    "Codewhale là mã nguồn mở. Đọc mã, tạo plugin, chia sẻ quy trình hoặc giúp cải thiện bản phát hành tiếp theo.",
  communityLinksAria: "Liên kết cộng đồng",
  contribute: "Đóng góp trên GitHub",
};
