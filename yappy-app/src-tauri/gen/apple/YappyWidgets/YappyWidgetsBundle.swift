// Widget Extension bundle entry point. WidgetKit looks for a struct annotated
// with @main; we point it at our YappyWidgetsBundle which exposes both:
//   - The Live Activity for in-progress audiobook renders (Lock Screen /
//     Dynamic Island, requires iOS 16.2).
//   - The home-screen widget (small + medium, iOS 16.0).

import WidgetKit
import SwiftUI

@main
struct YappyWidgetsBundle: WidgetBundle {
    var body: some Widget {
        if #available(iOS 16.2, *) {
            YappyRenderActivityWidget()
        }
        if #available(iOS 16.0, *) {
            YappyHomeWidget()
        }
    }
}
