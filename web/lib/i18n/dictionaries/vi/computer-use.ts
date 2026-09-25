import type { ComputerUseDict } from "../types";

/**
 * Vietnamese dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them; the macOS setting and folder names use the Vietnamese
 * system UI names (Trợ năng, Ghi màn hình, Ứng dụng).
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use cho Mac · Codewhale",
  metaDescription: "Tải và thiết lập Codewhale Computer Use cho Mac. Điều khiển ứng dụng trong nền, thiết lập quyền truy cập, cùng nút Pause và Stop do bạn kiểm soát.",
  title: "Computer Use",
  lead: "Để Codewhale làm việc trong các ứng dụng của bạn trong khi bạn vẫn tiếp tục công việc. Trợ lý cho Mac đưa quyền truy cập, điều khiển ứng dụng trong nền và cách tạm dừng hoặc dừng thao tác nhập lên thanh menu.",
  publisher: "Từ Codewhale",
  download: "Tải cho Mac",
  downloadZip: "Tệp ZIP (dùng cho trình cập nhật trong ứng dụng)",
  requirements: "macOS 13.5 trở lên · Apple silicon và Intel",
  included: "Chỉ cần tải một ứng dụng. Không cần cài riêng Node hay trình biên dịch.",
  pendingTitle: "Bản tải cho Mac đang được chuẩn bị",
  pendingBody: "Trình cài đặt công khai sẽ xuất hiện ở đây sau khi hoàn tất công chứng của Apple và các bước kiểm tra phát hành.",
  unavailableTitle: "Không kiểm tra được tình trạng tải xuống",
  unavailableBody: "Hãy tải lại trang để thử lại, hoặc xem các bản phát hành bên dưới.",
  releases: "Bản phát hành đã công bố",
  receipt: "Thông tin xác minh bản tải",
  setup: "Thiết lập máy Mac của bạn",
  steps: [
    { title: "Cài đặt ứng dụng", body: "Mở ảnh đĩa và kéo Codewhale Computer Use vào thư mục “Ứng dụng”. Mở ứng dụng từ “Ứng dụng”, rồi chọn Computer Use từ biểu tượng cá voi trên thanh menu." },
    { title: "Xem lại quyền truy cập", body: "Dùng các nút thiết lập để mở “Trợ năng” và “Ghi màn hình” trong Cài đặt Hệ thống. Bạn tự quyết định cấp những quyền nào." },
    { title: "Chạy kiểm tra nền", body: "Trợ lý mở một cửa sổ thử nghiệm dùng một lần, nhập văn bản và chụp cửa sổ đó, đồng thời kiểm tra xem con trỏ hoặc ứng dụng đang hoạt động có thay đổi trong lúc chạy hay không." },
    { title: "Kết nối với Codewhale", body: "Xem lại, tin cậy và bật Computer Use trong chợ plugin của Codewhale. Hãy dùng plugin 0.3.1 trở lên để các thao tác cục bộ đi qua nút Pause và Stop của trợ lý." },
  ],
  controlsTitle: "Tiếp tục làm việc. Giữ quyền kiểm soát.",
  controlsBody: "Các thao tác được hỗ trợ chạy trong nền trên ứng dụng đã chọn. Ứng dụng và cử chỉ cần điều khiển ở nền trước phải được bạn cho phép. Menu hiển thị ứng dụng mục tiêu và chế độ nhập; Pause (tạm dừng) ngưng thao tác nhập của trợ lý, còn Stop (dừng) kết thúc các phiên hiện có của nó.",
  updateTitle: "Cập nhật khi bạn muốn",
  updateBody: "Chọn Check for updates (kiểm tra cập nhật) trong ứng dụng. Trước khi cài bản cập nhật, ứng dụng kiểm tra tệp tải về, chữ ký Codewhale và công chứng của Apple, đồng thời giữ lại bản cũ để khôi phục khi cần.",
  help: "Thiết lập và khắc phục sự cố",
  notes: "Ghi chú phát hành",
  demo: "Xem kiểm tra nền",
  source: "Mã nguồn và các nền tảng khác",
  platforms: "Bản tải này dành cho Mac. Windows và Linux hiện dùng plugin từ mã nguồn cùng thiết lập phía máy chủ.",
};
