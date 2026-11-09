import AppKit
import WebKit

let args = CommandLine.arguments
guard args.count >= 3, let url = URL(string: args[1]) else {
    FileHandle.standardError.write("uso: snapshot.swift URL saida.png [largura] [altura] [espera_s]\n".data(using: .utf8)!)
    exit(2)
}
let out = URL(fileURLWithPath: args[2])
let width = args.count > 3 ? Double(args[3]) ?? 400 : 400
let height = args.count > 4 ? Double(args[4]) ?? 700 : 700
let wait = args.count > 5 ? Double(args[5]) ?? 1.5 : 1.5

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let frame = NSRect(x: -10000, y: -10000, width: width, height: height)
let window = NSWindow(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: height))
window.contentView = web
window.orderBack(nil)
web.load(URLRequest(url: url))

DispatchQueue.main.asyncAfter(deadline: .now() + wait) {
    web.takeSnapshot(with: nil) { image, error in
        guard let image, let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff),
              let png = rep.representation(using: .png, properties: [:]) else {
            FileHandle.standardError.write("falhou: \(String(describing: error))\n".data(using: .utf8)!)
            exit(1)
        }
        try? png.write(to: out)
        print("ok -> \(out.path)")
        exit(0)
    }
}
app.run()
