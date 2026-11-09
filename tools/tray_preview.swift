// Mostra o ícone da bandeja como o macOS desenha: NSImage "template" com 1x/2x/3x num
// botão sem borda, nos temas claro (Aqua) e escuro (Dark Aqua), normal e com alerta.
// Não precisa de permissão de gravação de tela (desenha fora da tela).
// Uso: swift tools/tray_preview.swift src-tauri/icons/tray saida.png
import AppKit

let dir = CommandLine.arguments[1]
let out = URL(fileURLWithPath: CommandLine.arguments[2])
_ = NSApplication.shared

func template(_ base: String) -> NSImage {
    let image = NSImage(size: NSSize(width: 16, height: 16))
    for suffix in ["", "@2x", "@3x"] {
        let data = try! Data(contentsOf: URL(fileURLWithPath: "\(dir)/\(base)\(suffix).png"))
        let rep = NSBitmapImageRep(data: data)!
        rep.size = NSSize(width: 16, height: 16)
        image.addRepresentation(rep)
    }
    image.isTemplate = true
    return image
}

// Duas faixas (claro e escuro), cada uma com o ícone normal e o de alerta, em 2x.
let cell = NSSize(width: 44, height: 24)
let canvas = NSView(frame: NSRect(x: 0, y: 0, width: cell.width * 2, height: cell.height * 2))
for (row, appearance) in [NSAppearance.Name.aqua, .darkAqua].enumerated() {
    let bar = NSView(frame: NSRect(x: 0, y: CGFloat(1 - row) * cell.height, width: cell.width * 2, height: cell.height))
    bar.wantsLayer = true
    bar.appearance = NSAppearance(named: appearance)
    bar.layer?.backgroundColor = (row == 0 ? NSColor(white: 0.93, alpha: 1) : NSColor(white: 0.12, alpha: 1)).cgColor
    for (col, base) in ["trayTemplate", "trayAlertTemplate"].enumerated() {
        let button = NSButton(frame: NSRect(x: CGFloat(col) * cell.width, y: 0, width: cell.width, height: cell.height))
        button.isBordered = false
        button.image = template(base)
        button.imagePosition = .imageOnly
        bar.addSubview(button)
    }
    canvas.addSubview(bar)
}
let window = NSWindow(contentRect: canvas.frame, styleMask: .borderless, backing: .buffered, defer: false)
window.contentView = canvas
window.setFrameOrigin(NSPoint(x: -10000, y: -10000))
window.orderBack(nil)
canvas.displayIfNeeded()
let rep = canvas.bitmapImageRepForCachingDisplay(in: canvas.bounds)!
canvas.cacheDisplay(in: canvas.bounds, to: rep)
try! rep.representation(using: .png, properties: [:])!.write(to: out)
print("ok -> \(out.path)")
