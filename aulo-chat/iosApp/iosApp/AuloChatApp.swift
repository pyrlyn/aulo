import AuloChat
import SwiftUI

@main
struct AuloChatApp: App {
    var body: some Scene {
        WindowGroup {
            ChatView()
                // Compose draws under the status bar and the home indicator and insets itself.
                .ignoresSafeArea()
        }
    }
}

/// The whole screen is the Compose view controller from the Kotlin framework.
struct ChatView: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> UIViewController {
        MainViewControllerKt.MainViewController()
    }

    func updateUIViewController(_ controller: UIViewController, context: Context) {}
}
