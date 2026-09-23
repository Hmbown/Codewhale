<!-- source: README.md sha256:29c349b6f2b4 -->
# Codewhale

Codewhale là tác nhân mã nguồn mở có thể đọc dự án, chỉnh sửa tệp, chạy lệnh và kiểm tra công việc của mình bằng mô hình do nhà cung cấp lưu trữ hoặc mô hình cục bộ mà bạn chọn. Hãy bắt đầu với một tác vụ trong terminal. Với công việc lớn hơn, bạn có thể giao từng phần cho các tác nhân dùng mô hình và đảm nhiệm vai trò khác nhau.

![Codewhale đang chạy trong terminal](web/public/codewhale-tui-d7a9a1c.png)

*Hình xem trước terminal từ bản dựng phát triển v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Cài đặt

Để cài mới trên macOS hoặc Linux, hãy dùng bản phát hành chính thức từ GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

Trình cài đặt chọn bản phát hành mới nhất đã được công bố. [Nhật ký thay đổi](CHANGELOG.md) cũng mô tả bản ứng viên chưa công bố của lần phát hành tiếp theo; những thay đổi đó chỉ có trong các bản tải xuống công khai khi bản phát hành tương ứng được công bố.

Trên Windows, tải bộ cài hoặc gói lưu trữ phù hợp từ [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Với bản cài trực tiếp đã có, chạy `codewhale update`; dùng `codewhale update --check` nếu chỉ muốn kiểm tra. Trình cập nhật hiển thị đường dẫn tệp thực thi và giữ lại các bản dựng mới hơn. npm và Cargo là lựa chọn phụ; xem [hướng dẫn cài đặt](docs/INSTALL.md) để chuyển từ trình quản lý gói và thiết lập PATH.

Trong lần chạy đầu tiên, Codewhale sẽ giúp bạn kết nối với nhà cung cấp hoặc cấu hình Codewhale ngoại tuyến. Để nhận phản hồi từ mô hình, bạn cần kết nối với mô hình do nhà cung cấp lưu trữ hoặc mô hình cục bộ. Codewhale cũng hỗ trợ npm và Cargo như các hình thức đóng gói thứ cấp, cùng với Docker, Nix, Scoop, Android/Termux và bản sao CNB tùy chọn. Các bản cài đặt hiện có qua trình quản lý gói sẽ được hướng dẫn chuyển đổi. Xem [trợ giúp cài đặt và PATH](docs/INSTALL.md).

Mỗi shell chỉ cần một lệnh để bật tính năng hoàn thành bằng phím Tab — `codewhale completion bash|zsh|fish|powershell|elvish`. Xem [tính năng hoàn thành của shell](docs/INSTALL.md#8-shell-completions).

## Sử dụng

Mở terminal trong thư mục dự án và chạy `codewhale`. Chọn nhà cung cấp bằng `/provider` và mô hình bằng `/model`. Sau đó mô tả một tác vụ cụ thể:

```text
Fix the failing tests and explain what changed.
```

Hoặc chạy tác vụ mà không cần mở TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale có thể đọc kho mã nguồn, chỉnh sửa tệp, chạy lệnh, kiểm tra kết quả và tiếp tục làm việc hướng đến mục tiêu. Dùng `/mode plan` để tìm hiểu mà không thay đổi tệp hay thực thi lệnh shell, và `/mode work` khi bạn muốn tác nhân thực hiện thay đổi. Nhấn `Shift+Tab` để chọn Ask, Auto-Review hoặc Full Access; [hướng dẫn về chế độ và quyền](docs/MODES.md) giải thích những thao tác được phép ở mỗi lựa chọn.

## Terminal, ứng dụng và Computer Use

Terminal và các ứng dụng khách đồ họa kết nối với Codewhale Runtime, nơi chạy tác nhân và các công cụ của nó:

- **Terminal:** `codewhale` mở giao diện tương tác; `codewhale exec` chạy tác vụ từ tập lệnh hoặc công việc CI.
- **Trình duyệt cục bộ:** `codewhale web` mở [ứng dụng web cục bộ](docs/WEB.md) đi kèm, dùng cùng Runtime.
- **Ứng dụng máy tính để bàn Codewhale (GPUI):** ứng dụng máy tính để bàn gốc GPUI là định hướng client sản phẩm (quyết định ngày 2026-09-14; bản đồ giai đoạn nằm trong docs/TRANSITION.md ở repo riêng tư codehwhale-gpui). Ứng dụng web lưu trữ tại app.codewhale.net sẽ ngừng theo từng giai đoạn; trang marketing, đăng nhập, thanh toán, pháp lý và tải xuống vẫn ở trên web vĩnh viễn. Thông tin về khả năng sử dụng được liệt kê trên [trang sản phẩm](https://codewhale.net/en/product).

**Computer Use bổ sung công cụ để quan sát và tương tác với các ứng dụng khác.** Plugin này có trong mã nguồn hiện tại. Hãy xem xét quyền truy cập được yêu cầu và bật plugin trước khi sử dụng; các yêu cầu về quyền của hệ điều hành và nền tảng vẫn được áp dụng. Xem [hướng dẫn Computer Use](crates/tui/plugins/computer-use/README.md) đi kèm và [thiết lập plugin](docs/PLUGINS.md).

Trong VS Code, tiện ích CodeWhale do cộng đồng duy trì kết nối với Runtime cục bộ từ thanh bên. Cài đặt từ [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); mã nguồn có trên [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Vì sao chọn Codewhale

- **Chọn mô hình của bạn.** Kết nối với nhà cung cấp dịch vụ hoặc với mô hình cục bộ thông qua Ollama, vLLM hay SGLang. Dùng `/provider` để đổi nhà cung cấp và `/model` để chọn mô hình.
- **Luôn nắm quyền kiểm soát.** Kiểm tra các thao tác được đề xuất và những thay đổi tệp do chúng tạo ra. Cài đặt phê duyệt quyết định khi nào cần xem xét; Full Access vẫn tuân thủ các giới hạn chính sách bắt buộc. `/undo` và `/restore` giúp khôi phục các thay đổi trong không gian làm việc.
- **Sắp xếp công việc dài hạn.** Lưu phiên, đặt `/goal` lâu dài, xem lại quy trình trước khi chạy và phối hợp các tác nhân mà không đưa chỉ dẫn nội bộ của chúng vào bản ghi hội thoại của bạn.
- **Mở rộng tác nhân bạn đang có.** Kết nối máy chủ MCP và kỹ năng, cấu hình hook, đồng thời lưu vai trò tác nhân dưới dạng các tệp dễ đọc trong dự án hoặc phần cài đặt cá nhân.

Chạy `/help` trong TUI để xem các lệnh và phím tắt.

## An toàn

Codewhale chạy trên máy của bạn với quyền truy cập do bạn cấp. Chế độ phê duyệt và quy tắc kho mã nguồn giới hạn những gì tác nhân được phép làm; cơ chế sandbox tùy chọn của hệ điều hành tạo thêm một ranh giới thực thi vững chắc hơn ở nơi được hỗ trợ. Giá mô hình chưa xác định sẽ vẫn được ghi là chưa xác định thay vì bị báo là miễn phí.

Đọc [thứ tự cấp quyền](docs/AUTHORIZATION_ORDER.md) để biết chính xác các lớp chính sách và [cấu hình](docs/CONFIGURATION.md) để biết các cài đặt cục bộ.

## Tài liệu

- [Nhà cung cấp và mô hình cục bộ](docs/PROVIDERS.md)
- [Nhóm tác nhân](docs/FLEET.md)
- [MCP](docs/MCP.md), [hook](docs/HOOKS.md) và [cấu hình](docs/CONFIGURATION.md)
- [Ứng dụng web cục bộ](docs/WEB.md)
- [Toàn bộ tài liệu](docs)
- [Cấu trúc kho mã và hướng dẫn đóng góp](CONTRIBUTING.md#project-structure)

## Tham gia cộng đồng

**Chúng tôi chào đón báo cáo lỗi, ý tưởng tính năng và pull request**, dù bạn đã dùng Codewhale nhiều tháng hay mới thử lần đầu. Nếu thiếu một nhà cung cấp, quy trình còn bất tiện hoặc giao diện terminal cản trở công việc, hãy [mở issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) hoặc [gửi pull request](CONTRIBUTING.md) để cùng cải thiện. Chúng tôi chào đón những đóng góp đầu tiên và luôn ghi nhận người đóng góp cho phần việc đã được hợp nhất.

Tham gia [Discord](https://discord.gg/37gfS3ksug), hoặc thêm Hunter trên WeChat (`hunterbown`) và đề nghị tham gia nhóm Whale Brothers.

## Lịch sử dự án

Codewhale bắt đầu với tên `deepseek-tui` và vẫn duy trì khả năng tương thích với cấu hình cùng phiên làm việc của dự án đó. Hiện nay Codewhale không phụ thuộc vào nhà cung cấp nào, được duy trì độc lập và không liên kết với bất kỳ nhà cung cấp mô hình nào.

Cảm ơn mọi người đóng góp và các cộng đồng mã nguồn mở đã giúp dự án phát triển. Xem [danh sách người đóng góp](docs/CONTRIBUTORS.md).

## Giấy phép

[MIT](LICENSE). Các phần được điều chỉnh từ những dự án nguồn mở khác được ghi trong [thông báo của bên thứ ba](docs/THIRD_PARTY_NOTICES.md).
