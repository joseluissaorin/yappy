// Widget Extension bundle entry point. WidgetKit looks for a struct annotated
// with @main; we point it at our YappyWidgetsBundle which exposes the single
// Live Activity widget configuration.

import WidgetKit
import SwiftUI

@main
struct YappyWidgetsBundle: WidgetBundle {
    var body: some Widget {
        if #available(iOS 16.2, *) {
            YappyRenderActivityWidget()
        }
    }
}
