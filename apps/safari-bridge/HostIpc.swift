import Foundation

/// Loopback IPC client matching Cookie Monster desktop ↔ native-host protocol.
/// Line-delimited JSON on 127.0.0.1:9735 with `token` from ipc.token.
enum HostIpc {
    static let defaultHost = "127.0.0.1"
    static let defaultPort: UInt16 = 9735
    static let timeoutSeconds: TimeInterval = 1.5

    static func tokenPath() -> URL {
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home
            .appendingPathComponent("Library/Application Support")
            .appendingPathComponent("com.CookieMonster.cookie-monster")
            .appendingPathComponent("ipc.token")
    }

    static func readToken() throws -> String {
        let path = tokenPath()
        let raw = try String(contentsOf: path, encoding: .utf8)
        let token = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !token.isEmpty else {
            throw IpcError.missingToken
        }
        return token
    }

    /// Forward a browser-extension request (without token) to the native host.
    static func forward(_ request: [String: Any]) throws -> [String: Any] {
        var body = request
        body["token"] = try readToken()

        let data = try JSONSerialization.data(withJSONObject: body, options: [])
        guard var line = String(data: data, encoding: .utf8) else {
            throw IpcError.encoding
        }
        line.append("\n")

        let port: UInt16 = {
            if let raw = ProcessInfo.processInfo.environment["COOKIE_MONSTER_TCP_PORT"],
               let value = UInt16(raw) {
                return value
            }
            return defaultPort
        }()
        let host = ProcessInfo.processInfo.environment["COOKIE_MONSTER_TCP_HOST"] ?? defaultHost

        let responseData = try tcpRequest(host: host, port: port, payload: line)
        guard let obj = try JSONSerialization.jsonObject(with: responseData) as? [String: Any] else {
            throw IpcError.badResponse
        }
        return obj
    }

    private static func tcpRequest(host: String, port: UInt16, payload: String) throws -> Data {
        var input: InputStream?
        var output: OutputStream?
        Stream.getStreamsToHost(withName: host, port: Int(port), inputStream: &input, outputStream: &output)
        guard let inputStream = input, let outputStream = output else {
            throw IpcError.connectFailed
        }

        inputStream.open()
        outputStream.open()
        defer {
            inputStream.close()
            outputStream.close()
        }

        let deadline = Date().addingTimeInterval(timeoutSeconds)
        while outputStream.streamStatus == .opening || inputStream.streamStatus == .opening {
            if Date() > deadline { throw IpcError.timeout }
            Thread.sleep(forTimeInterval: 0.01)
        }
        guard outputStream.streamStatus == .open, inputStream.streamStatus == .open else {
            throw IpcError.connectFailed
        }

        guard let payloadData = payload.data(using: .utf8) else { throw IpcError.encoding }
        var written = 0
        while written < payloadData.count {
            if Date() > deadline { throw IpcError.timeout }
            let n = payloadData.withUnsafeBytes { raw -> Int in
                guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return outputStream.write(base.advanced(by: written), maxLength: payloadData.count - written)
            }
            if n < 0 { throw IpcError.writeFailed }
            written += n
        }

        var buffer = [UInt8](repeating: 0, count: 65_536)
        var collected = Data()
        while Date() < deadline {
            if inputStream.hasBytesAvailable {
                let n = inputStream.read(&buffer, maxLength: buffer.count)
                if n < 0 { throw IpcError.readFailed }
                if n == 0 { break }
                collected.append(buffer, count: n)
                if collected.contains(UInt8(ascii: "\n")) { break }
            } else if inputStream.streamStatus == .atEnd {
                break
            } else {
                Thread.sleep(forTimeInterval: 0.01)
            }
        }

        guard let newline = collected.firstIndex(of: UInt8(ascii: "\n")) else {
            throw IpcError.timeout
        }
        return collected.subdata(in: collected.startIndex..<newline)
    }

    enum IpcError: Error, LocalizedError {
        case missingToken
        case encoding
        case connectFailed
        case writeFailed
        case readFailed
        case timeout
        case badResponse

        var errorDescription: String? {
            switch self {
            case .missingToken:
                return "ipc.token missing — start Cookie Monster desktop or native host first"
            case .encoding:
                return "failed to encode IPC payload"
            case .connectFailed:
                return "cannot connect to Cookie Monster host on loopback"
            case .writeFailed:
                return "failed writing to host"
            case .readFailed:
                return "failed reading from host"
            case .timeout:
                return "host IPC timed out"
            case .badResponse:
                return "host returned non-JSON response"
            }
        }
    }
}
