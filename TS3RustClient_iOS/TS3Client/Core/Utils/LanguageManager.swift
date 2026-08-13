// File: TS3RustClient_iOS/TS3Client/Core/Utils/LanguageManager.swift
//
// 应用内中英文实时切换（UserDefaults + Bundle swizzle，无需重启）。

import Foundation
import ObjectiveC.runtime

final class LanguageManager {

    static let shared = LanguageManager()
    static let didChangeNotification = Notification.Name("LanguageManager.didChange")

    private let languageKey = "AppLanguage"

    var currentLanguage: String {
        if let code = UserDefaults.standard.string(forKey: languageKey) {
            return code
        }
        let preferred = Locale.preferredLanguages.first ?? "en"
        let code = preferred.hasPrefix("zh") ? "zh-Hans" : "en"
        UserDefaults.standard.set(code, forKey: languageKey)
        return code
    }

    func setLanguage(_ code: String) {
        guard code != currentLanguage else { return }
        UserDefaults.standard.set(code, forKey: languageKey)
        UserDefaults.standard.set([code], forKey: "AppleLanguages")
        UserDefaults.standard.synchronize()
        NotificationCenter.default.post(name: Self.didChangeNotification, object: nil)
    }

    func localizedString(_ key: String) -> String {
        guard let path = Bundle.main.path(forResource: currentLanguage, ofType: "lproj"),
              let bundle = Bundle(path: path) else {
            return key
        }
        return bundle.localizedString(forKey: key, value: nil, table: nil)
    }
}

extension Bundle {

    private static let languageSwizzleKey = UnsafeRawPointer(bitPattern: 0x4C414E4753)!

    static func enableLanguageSwizzling() {
        guard objc_getAssociatedObject(self, languageSwizzleKey) == nil else { return }
        objc_setAssociatedObject(self, languageSwizzleKey, true, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)

        let original = class_getInstanceMethod(self, #selector(localizedString(forKey:value:table:)))
        let swizzled = class_getInstanceMethod(self, #selector(ts3_localizedString(forKey:value:table:)))
        if let original = original, let swizzled = swizzled {
            method_exchangeImplementations(original, swizzled)
        }
    }

    @objc private func ts3_localizedString(forKey key: String, value: String?, table: String?) -> String {
        let language = LanguageManager.shared.currentLanguage
        if let path = Bundle.main.path(forResource: language, ofType: "lproj"),
           let bundle = Bundle(path: path) {
            return bundle.localizedString(forKey: key, value: value, table: table)
        }
        return ts3_localizedString(forKey: key, value: value, table: table)
    }
}
