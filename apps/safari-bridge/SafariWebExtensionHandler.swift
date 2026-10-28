import SafariServices
import os.log

/// Safari Web Extension native handler.
/// Receives messages from the extension JS and forwards them to cookie-monster-native-host
/// over the same loopback JSON protocol the desktop app uses.
class SafariWebExtensionHandler: NSObject, NSExtensionRequestHandling {
    private let log = Logger(subsystem: "com.cookiemonster.safari", category: "bridge")

    func beginRequest(with context: NSExtensionContext) {
        let message = extractMessage(from: context) ?? [:]
        log.debug("safari bridge request keys=\(message.keys.sorted().joined(separator: ","), privacy: .public)")

        do {
            let response = try HostIpc.forward(message)
            complete(context, with: response)
        } catch {
            log.error("safari bridge error: \(error.localizedDescription, privacy: .public)")
            complete(context, with: [
                "type": "error",
                "message": error.localizedDescription,
            ])
        }
    }

    private func extractMessage(from context: NSExtensionContext) -> [String: Any]? {
        guard
            let item = context.inputItems.first as? NSExtensionItem,
            let userInfo = item.userInfo as? [String: Any]
        else {
            return nil
        }

        // Safari uses SFExtensionMessageKey; fall back to common string keys used by converter templates.
        if let message = userInfo[SFExtensionMessageKey] as? [String: Any] {
            return message
        }
        if let message = userInfo["message"] as? [String: Any] {
            return message
        }
        return nil
    }

    private func complete(_ context: NSExtensionContext, with payload: [String: Any]) {
        let response = NSExtensionItem()
        response.userInfo = [SFExtensionMessageKey: payload]
        context.completeRequest(returningItems: [response], completionHandler: nil)
    }
}
