// Print matching on-screen windows as TSV:
//   id  owner  title  x  y  width  height
// Usage: swift window.swift <owner substring> [title substring]
import CoreGraphics
import Foundation

let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write(Data("usage: window.swift <owner> [title]\n".utf8))
    exit(2)
}
let ownerNeedle = args[1].lowercased()
let titleNeedle = args.count >= 3 ? args[2].lowercased() : ""
let options = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows {
    let owner = window[kCGWindowOwnerName as String] as? String ?? ""
    let title = window[kCGWindowName as String] as? String ?? ""
    let layer = window[kCGWindowLayer as String] as? Int ?? 0
    guard layer == 0, owner.lowercased().contains(ownerNeedle) else { continue }
    if !titleNeedle.isEmpty && !title.lowercased().contains(titleNeedle) { continue }
    let id = window[kCGWindowNumber as String] as? Int ?? 0
    let bounds = window[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let x = (bounds["X"] as? Double) ?? 0
    let y = (bounds["Y"] as? Double) ?? 0
    let w = (bounds["Width"] as? Double) ?? 0
    let h = (bounds["Height"] as? Double) ?? 0
    print("\(id)\t\(owner)\t\(title)\t\(Int(x))\t\(Int(y))\t\(Int(w))\t\(Int(h))")
}
