import Foundation
import WebKit
import AppKit

let isDark = CommandLine.arguments.contains("--dark")

// Read Mermaid code from stdin
let inputData = FileHandle.standardInput.readDataToEndOfFile()
guard let rawCode = String(data: inputData, encoding: .utf8), !rawCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
    exit(0)
}

func sanitizeMermaid(_ input: String) -> String {
    var result = input
    // Auto-quote database cylindrical nodes containing parentheses: [(label (info))] -> [("label (info)")]
    if let dbPattern = try? NSRegularExpression(pattern: #"\[\(([^\"]*?\([^\"]*?\)[^\"]*?)\)\]"#, options: []) {
        result = dbPattern.stringByReplacingMatches(
            in: result,
            options: [],
            range: NSRange(location: 0, length: result.utf16.count),
            withTemplate: "[(\"$1\")]"
        )
    }
    // Auto-quote standard rectangular nodes containing parentheses: [label (info)] -> ["label (info)"]
    if let rectPattern = try? NSRegularExpression(pattern: #"(?<!\()\[([^\"]*?\([^\"]*?\)[^\"]*?)\](?!\))"#, options: []) {
        result = rectPattern.stringByReplacingMatches(
            in: result,
            options: [],
            range: NSRange(location: 0, length: result.utf16.count),
            withTemplate: "[\"$1\"]"
        )
    }
    return result
}

let sanitizedCode = sanitizeMermaid(rawCode)

// Locate mermaid.min.js
func findMermaidJs() -> String? {
    let candidatePaths: [String?] = [
        Bundle.main.path(forResource: "mermaid.min", ofType: "js"),
        Bundle.main.resourcePath.map { "\($0)/mermaid.min.js" },
        Bundle.main.executablePath.map { URL(fileURLWithPath: $0).deletingLastPathComponent().appendingPathComponent("mermaid.min.js").path },
        Bundle.main.executablePath.map { URL(fileURLWithPath: $0).deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("Resources/mermaid.min.js").path },
        "/tmp/mermaid.min.js",
        "crates/glidemark_app/assets/mermaid.min.js"
    ]

    for candidate in candidatePaths.compactMap({ $0 }) {
        if FileManager.default.fileExists(atPath: candidate),
           let content = try? String(contentsOf: URL(fileURLWithPath: candidate), encoding: .utf8) {
            return content
        }
    }
    return nil
}

guard let mermaidJs = findMermaidJs() else {
    fputs("failed to locate mermaid.min.js\n", stderr)
    exit(1)
}

let theme = isDark ? "dark" : "default"
let html = """
<!DOCTYPE html><html><head><meta charset="utf-8"></head><body>
<div id="diagram"></div>
<script>\(mermaidJs)</script>
<script>mermaid.initialize({ startOnLoad: false, theme: '\(theme)', securityLevel: 'loose' });</script>
</body></html>
"""

class AppDelegate: NSObject, NSApplicationDelegate, WKNavigationDelegate {
    var webView: WKWebView!
    let code: String

    init(code: String) {
        self.code = code
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let config = WKWebViewConfiguration()
        webView = WKWebView(frame: CGRect(x: 0, y: 0, width: 1400, height: 900), configuration: config)
        webView.navigationDelegate = self
        webView.loadHTMLString(html, baseURL: nil)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        let js = """
        const { svg } = await mermaid.render('d1', \(code.debugDescription));
        return svg;
        """
        webView.callAsyncJavaScript(js, arguments: [:], in: nil, in: .page) { (result) in
            switch result {
            case .success(let val):
                if let svg = val as? String {
                    FileHandle.standardOutput.write(svg.data(using: .utf8)!)
                }
            case .failure(let err):
                fputs("Mermaid Render Error: \(err)\n", stderr)
                exit(2)
            }
            NSApplication.shared.terminate(nil)
        }
    }
}

let app = NSApplication.shared
let delegate = AppDelegate(code: sanitizedCode)
app.delegate = delegate
app.run()
