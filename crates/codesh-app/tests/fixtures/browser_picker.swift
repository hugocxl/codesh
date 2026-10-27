import Cocoa
import WebKit

final class PickerTest: NSObject, WKNavigationDelegate {
    let page = WKWebView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))

    override init() {
        super.init()
        page.navigationDelegate = self
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        do {
            let body = try String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
            webView.callAsyncJavaScript(body, arguments: [:], in: nil, in: .page, completionHandler: { result in
                switch result {
                case .success(let value):
                    print(value ?? "null")
                    NSApplication.shared.terminate(nil)
                case .failure(let error):
                    print("WebKit picker test failed: \(error)")
                    exit(1)
                }
            })
        } catch {
            print(error)
            exit(1)
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let test = PickerTest()
test.page.loadHTMLString("<!doctype html><html><body></body></html>", baseURL: nil)
DispatchQueue.main.asyncAfter(deadline: .now() + 20) {
    print("WebKit picker test timed out")
    exit(1)
}
app.run()
