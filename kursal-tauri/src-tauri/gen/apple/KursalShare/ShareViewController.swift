import UIKit
import UniformTypeIdentifiers
import os

private let logger = Logger(subsystem: "chat.kursal.share", category: "share")

private struct ShareFile: Encodable {
  let path: String
  let filename: String
  let sizeBytes: UInt64
}

private struct SharePayload: Encodable {
  let files: [ShareFile]
  let text: String?
}

// Streams shared items into the app group container and writes manifest.json last,
// via a move, so take_pending_shares never sees a half-copied payload.
final class ShareViewController: UIViewController {
  private static let appGroup = "group.chat.kursal"
  private static let shareDir = "kursal-shares"
  private static let manifest = "manifest.json"

  private let spinner = UIActivityIndicatorView(style: .large)
  private let notice = UILabel()

  override func viewDidLoad() {
    super.viewDidLoad()

    view.backgroundColor = UIColor.systemBackground.withAlphaComponent(0.9)

    notice.translatesAutoresizingMaskIntoConstraints = false
    notice.textAlignment = .center
    notice.numberOfLines = 0
    notice.font = .preferredFont(forTextStyle: .body)
    notice.textColor = .label
    notice.isHidden = true

    spinner.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(spinner)
    view.addSubview(notice)
    NSLayoutConstraint.activate([
      spinner.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      spinner.centerYAnchor.constraint(equalTo: view.centerYAnchor),
      notice.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      notice.centerYAnchor.constraint(equalTo: view.centerYAnchor),
      notice.leadingAnchor.constraint(greaterThanOrEqualTo: view.leadingAnchor, constant: 32),
      notice.trailingAnchor.constraint(lessThanOrEqualTo: view.trailingAnchor, constant: -32),
    ])
    spinner.startAnimating()

    Task { await self.run() }
  }

  private func run() async {
    let providers = (extensionContext?.inputItems as? [NSExtensionItem] ?? [])
      .flatMap { $0.attachments ?? [] }

    guard let dir = Self.makeStagingDir() else {
      logger.error("app group \(Self.appGroup, privacy: .public) unavailable, nothing staged")
      finishWithNotice("Kursal could not save what you shared.")
      return
    }

    var files: [ShareFile] = []
    var texts: [String] = []

    for (index, provider) in providers.enumerated() {
      if let text = await Self.loadText(provider) {
        texts.append(text)
      } else if let file = await Self.stageFile(provider, into: dir, index: index) {
        files.append(file)
      }
    }

    let text = texts.isEmpty ? nil : texts.joined(separator: "\n")

    guard !files.isEmpty || text != nil else {
      logger.error("none of the \(providers.count, privacy: .public) shared items could be read")
      try? FileManager.default.removeItem(at: dir)
      finishWithNotice("Kursal could not read what you shared.")
      return
    }

    guard Self.writeManifest(SharePayload(files: files, text: text), into: dir) else {
      logger.error("could not write share manifest")
      try? FileManager.default.removeItem(at: dir)
      finishWithNotice("Kursal could not save what you shared.")
      return
    }

    logger.log("staged \(files.count, privacy: .public) file(s), text: \(text != nil)")
    openHostApp()
  }

  private func openHostApp() {
    guard let url = URL(string: "kursal://share") else {
      finish()
      return
    }

    // Opening the host app from a share extension is undocumented and does not
    // always work. When it is refused the payload stays queued in the container,
    // so the extension says so rather than dismissing as if nothing happened.
    extensionContext?.open(url) { [weak self] opened in
      logger.log("extensionContext.open(kursal://share) returned \(opened, privacy: .public)")
      DispatchQueue.main.async {
        if opened {
          self?.finish()
        } else {
          self?.finishWithNotice("Saved to Kursal.\nOpen Kursal to choose who to send it to.")
        }
      }
    }
  }

  private func finishWithNotice(_ message: String) {
    spinner.stopAnimating()
    spinner.isHidden = true
    notice.text = message
    notice.isHidden = false

    DispatchQueue.main.asyncAfter(deadline: .now() + 2.2) { [weak self] in
      self?.finish()
    }
  }

  private func finish() {
    spinner.stopAnimating()
    extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
  }

  private static func makeStagingDir() -> URL? {
    guard
      let container = FileManager.default.containerURL(
        forSecurityApplicationGroupIdentifier: appGroup)
    else { return nil }

    let dir = container
      .appendingPathComponent(shareDir, isDirectory: true)
      .appendingPathComponent(UUID().uuidString.lowercased(), isDirectory: true)

    do {
      try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
      return dir
    } catch {
      return nil
    }
  }

  private static func writeManifest(_ payload: SharePayload, into dir: URL) -> Bool {
    let temp = dir.appendingPathComponent("\(manifest).tmp")
    let target = dir.appendingPathComponent(manifest)

    do {
      try JSONEncoder().encode(payload).write(to: temp)
      try FileManager.default.moveItem(at: temp, to: target)
      return true
    } catch {
      return false
    }
  }

  private static func loadText(_ provider: NSItemProvider) async -> String? {
    if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) { return nil }

    let textTypes = [UTType.plainText.identifier, UTType.url.identifier]
    guard let type = textTypes.first(where: provider.hasItemConformingToTypeIdentifier) else {
      return nil
    }

    return await withCheckedContinuation { continuation in
      provider.loadItem(forTypeIdentifier: type, options: nil) { item, _ in
        switch item {
        case let value as String: continuation.resume(returning: value)
        case let value as URL: continuation.resume(returning: value.absoluteString)
        case let value as NSAttributedString: continuation.resume(returning: value.string)
        default: continuation.resume(returning: nil)
        }
      }
    }
  }

  private static func stageFile(_ provider: NSItemProvider, into dir: URL, index: Int) async
    -> ShareFile?
  {
    await withCheckedContinuation { continuation in
      provider.loadFileRepresentation(forTypeIdentifier: UTType.item.identifier) { url, _ in
        guard let url else {
          continuation.resume(returning: nil)
          return
        }

        let name = sanitize(
          provider.suggestedName ?? url.lastPathComponent,
          index: index,
          fallbackExtension: url.pathExtension)
        let target = dir.appendingPathComponent(name)

        do {
          if FileManager.default.fileExists(atPath: target.path) {
            try FileManager.default.removeItem(at: target)
          }
          try FileManager.default.copyItem(at: url, to: target)

          let size = (try? FileManager.default.attributesOfItem(atPath: target.path)[.size])
            .flatMap { $0 as? UInt64 } ?? 0

          continuation.resume(
            returning: ShareFile(path: target.path, filename: name, sizeBytes: size))
        } catch {
          logger.error("could not copy shared item \(name, privacy: .public): \(error)")
          continuation.resume(returning: nil)
        }
      }
    }
  }

  private static func sanitize(_ name: String, index: Int, fallbackExtension: String) -> String {
    let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz")
      .union(CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-"))

    let safe = String(
      name.trimmingCharacters(in: .whitespacesAndNewlines).unicodeScalars.map {
        allowed.contains($0) ? Character($0) : "_"
      })

    guard !safe.isEmpty, safe != "." else {
      let ext = fallbackExtension.isEmpty ? "bin" : fallbackExtension
      return "shared-\(index + 1).\(ext)"
    }

    if (safe as NSString).pathExtension.isEmpty, !fallbackExtension.isEmpty {
      return "\(safe).\(fallbackExtension)"
    }

    return safe
  }
}
