// File: TS3RustClient_iOS/TS3Client/UI/MainTabBarController.swift

import UIKit

final class MainTabBarController: UITabBarController {

    override func viewDidLoad() {
        super.viewDidLoad()

        let connectVC = UINavigationController(rootViewController: ConnectViewController())
        connectVC.tabBarItem = UITabBarItem(
            title: LanguageManager.shared.localizedString("connect"),
            image: UIImage(systemName: "antenna.radiowaves.left.and.right"),
            tag: 0
        )

        let settingsVC = UINavigationController(rootViewController: SettingsViewController())
        settingsVC.tabBarItem = UITabBarItem(
            title: LanguageManager.shared.localizedString("settings_language"),
            image: UIImage(systemName: "gearshape"),
            tag: 1
        )

        viewControllers = [connectVC, settingsVC]
    }
}
