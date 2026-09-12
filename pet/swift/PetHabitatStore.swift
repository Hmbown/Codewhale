import Foundation
import CryptoKit
import Darwin

/// A private, bounded sidecar for each source. The anchored directory, advisory
/// lock and content revision preserve a newer or damaged file on every failure.
final class PetHabitatStore {
    static let limit = 8 * 1024 * 1024
    private let directory: Int32
    private let lock: Int32
    private let name: String
    private let lockName: String
    private var expected: Data?
    private var loaded = false

    init(directory url: URL, source: String) throws {
        guard ["wild", "demo", "live"].contains(source) else { throw Self.invalid("Invalid pet source.") }
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let root = open(url.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard root >= 0 else { throw Self.ioError() }
        let key = source + ".lock"
        let file = openat(root, key, O_RDWR | O_CREAT | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC, 0o600)
        guard file >= 0 else { let error = Self.ioError(); close(root); throw error }
        do { _ = try Self.regular(file) }
        catch { close(file); close(root); throw error }
        self.directory = root; self.lock = file; self.name = source + ".json"; self.lockName = key
    }
    deinit { close(lock); close(directory) }

    func load() throws -> Data? {
        let value = try coordinated { try read() }
        expected = value.map { Data(SHA256.hash(data: $0)) }; loaded = true
        return value
    }

    func save(_ value: Data) throws {
        guard loaded, value.count <= Self.limit else { throw Self.invalid("The pet habitat cannot be saved within its size limit.") }
        try coordinated {
            let current = try read().map { Data(SHA256.hash(data: $0)) }
            guard current == expected else { throw Self.invalid("Another writer changed the saved habitat.") }
            let temporary = ".pet-" + UUID().uuidString
            let file = openat(directory, temporary, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
            guard file >= 0 else { throw Self.ioError() }
            defer { close(file); unlinkat(directory, temporary, 0) }
            try value.withUnsafeBytes { bytes in
                var offset = 0
                while offset < bytes.count {
                    let written = Darwin.write(file, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
                    if written < 0 && errno == EINTR { continue }
                    guard written > 0 else { throw Self.ioError() }
                    offset += written
                }
            }
            guard fsync(file) == 0, renameat(directory, temporary, directory, name) == 0 else { throw Self.ioError() }
        }
        expected = Data(SHA256.hash(data: value))
    }

    private func coordinated<T>(_ action: () throws -> T) throws -> T {
        guard flock(lock, LOCK_EX | LOCK_NB) == 0 else { throw Self.ioError() }
        defer { flock(lock, LOCK_UN) }
        let current = openat(directory, lockName, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        guard current >= 0 else { throw Self.ioError() }
        defer { close(current) }
        let original = try Self.regular(lock), now = try Self.regular(current)
        guard original.st_dev == now.st_dev && original.st_ino == now.st_ino else { throw Self.invalid("The habitat writer lock was replaced.") }
        return try action()
    }

    private func read() throws -> Data? {
        let file = openat(directory, name, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        if file < 0 && errno == ENOENT { return nil }
        guard file >= 0 else { throw Self.ioError() }
        defer { close(file) }
        let info = try Self.regular(file)
        guard info.st_size >= 0 && info.st_size <= Self.limit else { throw Self.invalid("The saved habitat exceeds 8 MiB.") }
        var value = Data(), buffer = [UInt8](repeating: 0, count: 16_384)
        while true {
            let count = Darwin.read(file, &buffer, min(buffer.count, Self.limit + 1 - value.count))
            if count < 0 && errno == EINTR { continue }
            guard count >= 0 else { throw Self.ioError() }
            if count == 0 { break }
            value.append(contentsOf: buffer.prefix(count))
            guard value.count <= Self.limit else { throw Self.invalid("The saved habitat exceeds 8 MiB.") }
        }
        return value
    }

    private static func regular(_ file: Int32) throws -> stat {
        var info = stat()
        guard fstat(file, &info) == 0 else { throw ioError() }
        guard info.st_mode & S_IFMT == S_IFREG, info.st_nlink == 1 else { throw invalid("A habitat file must be a regular file without links.") }
        return info
    }
    private static func ioError() -> Error { NSError(domain: NSPOSIXErrorDomain, code: Int(errno)) }
    private static func invalid(_ message: String) -> Error { NSError(domain: "CodewhalePet", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
}
