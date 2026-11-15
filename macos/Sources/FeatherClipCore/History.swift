import CryptoKit
import Foundation

public enum ClipContent: Equatable {
    case text(String)
    case image(Data, String)
    case files([URL])

    public var bytes: Int {
        switch self {
        case .text(let text): return text.utf8.count
        case .image(let data, _): return data.count
        case .files(let urls): return urls.reduce(0) { $0 + $1.absoluteString.utf8.count }
        }
    }
    public var valid: Bool {
        switch self {
        case .text(let text):
            return bytes <= 102400 && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .image(let data, let type):
            return !data.isEmpty && bytes <= 8 * 1024 * 1024
                && ["public.png", "public.tiff"].contains(type)
        case .files(let urls):
            return !urls.isEmpty && urls.count <= 100 && bytes <= 102400
                && urls.allSatisfy(\.isFileURL)
        }
    }
    public var label: String {
        switch self {
        case .text(let text): return text
        case .image(let data, let type):
            return
                "Image · \(type == "public.png" ? "PNG" : "TIFF") · \((data.count + 1023) / 1024) KiB"
        case .files(let urls):
            return (urls.count > 1 ? "\(urls.count) files\n" : "")
                + urls.map(\.lastPathComponent).joined(separator: ", ")
        }
    }
    var fingerprint: Data {
        var hash = SHA256()
        switch self {
        case .text(let text):
            hash.update(data: Data("text\0".utf8))
            hash.update(data: Data(text.utf8))
        case .image(let data, let type):
            hash.update(data: Data(type.utf8))
            hash.update(data: data)
        case .files(let urls):
            hash.update(data: Data("files\0".utf8))
            for url in urls {
                hash.update(data: Data(url.absoluteString.utf8))
                hash.update(data: Data([0]))
            }
        }
        return Data(hash.finalize())
    }
}

public struct Clip: Equatable {
    public let id: Int
    public let content: ClipContent
    public var label: String { content.label }
}

public final class History {
    public private(set) var items: [Clip] = []
    public private(set) var paused = false
    private var lastSeen: Data?
    private var baseline = true
    private var nextID = 0
    public init() {}

    @discardableResult public func observe(_ value: ClipContent?) -> Bool {
        guard !paused else { return false }
        let content = value.flatMap { $0.valid ? $0 : nil }
        let fingerprint = content?.fingerprint
        let changed = fingerprint != lastSeen
        lastSeen = fingerprint
        if baseline {
            baseline = false
            return false
        }
        guard changed, let content else { return false }
        return insert(content)
    }
    @discardableResult public func insert(_ content: ClipContent) -> Bool {
        guard content.valid, items.first?.content != content else { return false }
        let clip: Clip
        if let index = items.firstIndex(where: { $0.content == content }) {
            clip = items.remove(at: index)
        } else {
            nextID += 1
            clip = Clip(id: nextID, content: content)
        }
        items.insert(clip, at: 0)
        while items.count > 30 || items.reduce(0, { $0 + $1.content.bytes }) > 24 * 1024 * 1024 {
            items.removeLast()
        }
        return true
    }
    public func copied(_ content: ClipContent) {
        lastSeen = content.fingerprint
        baseline = false
        insert(content)
    }
    public func togglePause() {
        paused.toggle()
        lastSeen = nil
        baseline = true
    }
    public func clear() {
        items.removeAll()
        lastSeen = nil
        baseline = true
    }
}
