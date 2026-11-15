import XCTest
import AppKit
@testable import FeatherClipCore

final class HistoryTests: XCTestCase {
    func testBaselinePauseClearAndSelection() {
        let history = History()
        XCTAssertFalse(history.observe(.text("existing private copy")))
        XCTAssertTrue(history.observe(.text("سلام 👋\nsecond line")))
        let id = history.items[0].id
        history.insert(.text("another")); history.copied(.text("سلام 👋\nsecond line"))
        XCTAssertEqual(history.items[0].id, id)
        XCTAssertFalse(history.observe(.text("سلام 👋\nsecond line")))
        history.togglePause(); XCTAssertFalse(history.observe(.text("paused secret")))
        history.togglePause(); XCTAssertFalse(history.observe(.text("paused secret")))
        XCTAssertTrue(history.observe(.text("new")))
        history.clear(); XCTAssertFalse(history.observe(.text("new")))
        XCTAssertTrue(history.observe(.text("first copy after clear")))
    }
    func testRetentionLimitsAndImageFingerprint() {
        let history = History()
        for n in 0..<40 { history.insert(.text(String(n))) }
        XCTAssertEqual(history.items.count, 30)
        XCTAssertFalse(history.insert(.text(String(repeating: "é", count: 51201))))
        history.clear(); history.observe(nil)
        for n in 0..<4 { history.observe(.image(Data(repeating: UInt8(n), count: 8 * 1024 * 1024), "public.png")) }
        XCTAssertEqual(history.items.count, 3)
        XCTAssertFalse(history.insert(.image(Data(repeating: 0, count: 8 * 1024 * 1024 + 1), "public.png")))
    }
    func testActualPrivatePasteboardAndConcealedExclusion() throws {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        let text = ClipContent.text("  سلام 👋\nexact\t ")
        try NativeClipboard.write(text, to: board)
        XCTAssertEqual(NativeClipboard.read(board), text)
        board.setString("", forType: .init("org.nspasteboard.ConcealedType"))
        XCTAssertNil(NativeClipboard.read(board))
        let image = ClipContent.image(Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAACN0lEQVR42mNiGGDANNAOYMEm+Pz9fYd//xjiGRgZAhgZGAQoseA/A8MHILGBiYlhoaSg4gF0eUZ0gadv789nZGRIoIVv//9nWCAtrJiI0wG0tByXIxiRgx0ouZ8e8Q70pCMsOuCJ8P8/wj5fsW8nQ2pvM0PN3Kk41Vy5f4fBv6aQoX3pPIZ7z59iDwUkuxAOYGTwx2f5iWuXGZbv28Hw6v07hi8/vuNU9xUqd+L6ZYZ52zZgdwCSXSxIcYE1tX/6+pHhxNXjQANvwcXuA31WMLWHgYeDE0M9sq8vA0MDaxQg2cVCKNgv3rnA8Pvvbwzx+ziClyrlAEoIfPs0AktCQqAlIY+Bh5MLp/yaw7sYjlw5RzsHLNi9AZgAcTvgzrNHtA2BO08fDWwUrKjuZZAQEsEp37FiDsOO04dp54CI1uKBDQEVaTmMNPDi/RuGF+/e0McBCW6BGLlgzaGd9HNAzfyJ9I8CXi4Okg0XExCiXkmoIC7KwMZCWoA5GZlSHgKc7JwM339+Z2BjZWGIcrJiePPpM0FDlaVUGBQk5RmUJKUpd4CqjDrDpbsXwGyQI6SEBfGq5+PiY7DQNmVgZWElOQ18BGJ+dAWyYrLAQkcC3C4gBECW8nHzE2PvR0wHAJvOwJZCPC6DhflFqNgyZdiAkQgZmRgW0KsKRrYL7gBwK/U/w0Ka2/4ftYOC0TF59vb+AlxRQQ3LpYQVE/D2jOB9BFDTGdg1w5YwSQQfQXEOCnaiumYjrncMAMftst56tpHCAAAAAElFTkSuQmCC")!, "public.png")
        try NativeClipboard.write(image, to: board)
        XCTAssertEqual(NativeClipboard.read(board), image)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let urls = ["one test.txt", "two.png"].map { directory.appendingPathComponent($0) }
        for url in urls { try Data("fixture".utf8).write(to: url) }
        try NativeClipboard.write(.files(urls), to: board)
        XCTAssertEqual(NativeClipboard.read(board), .files(urls))
    }
}
