// File: TS3RustClient_iOS/TS3Client/UI/Settings/SettingsViewController.swift

import UIKit

final class SettingsViewController: UIViewController {

    private let languageControl = UISegmentedControl(items: ["English", "中文"])
    private let tableView = UITableView(frame: .zero, style: .insetGrouped)

    override func viewDidLoad() {
        super.viewDidLoad()
        title = LanguageManager.shared.localizedString("settings_language")
        view.backgroundColor = .systemBackground

        languageControl.selectedSegmentIndex =
            LanguageManager.shared.currentLanguage == "zh-Hans" ? 1 : 0
        languageControl.addTarget(self, action: #selector(languageChanged(_:)), for: .valueChanged)

        tableView.dataSource = self
        tableView.delegate = self
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "cell")

        let stack = UIStackView(arrangedSubviews: [languageControl, tableView])
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false

        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            stack.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            languageControl.heightAnchor.constraint(equalToConstant: 36)
        ])
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        tableView.reloadData()
    }

    @objc private func languageChanged(_ sender: UISegmentedControl) {
        let code = sender.selectedSegmentIndex == 1 ? "zh-Hans" : "en"
        LanguageManager.shared.setLanguage(code)
        title = LanguageManager.shared.localizedString("settings_language")
        tableView.reloadData()
    }
}

extension SettingsViewController: UITableViewDataSource, UITableViewDelegate {

    func numberOfSections(in tableView: UITableView) -> Int {
        1
    }

    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        max(ServerConfigManager.shared.servers.count, 1)
    }

    func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        LanguageManager.shared.localizedString("saved_servers")
    }

    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "cell", for: indexPath)
        let servers = ServerConfigManager.shared.servers
        if servers.isEmpty {
            cell.textLabel?.text = LanguageManager.shared.localizedString("no_servers")
            cell.textLabel?.textColor = .secondaryLabel
        } else {
            let config = servers[indexPath.row]
            cell.textLabel?.text = "\(config.server):\(config.port) — \(config.nickname)"
            cell.textLabel?.textColor = .label
        }
        return cell
    }

    func tableView(_ tableView: UITableView,
                   commit editingStyle: UITableViewCell.EditingStyle,
                   forRowAt indexPath: IndexPath) {
        if editingStyle == .delete {
            ServerConfigManager.shared.remove(at: indexPath.row)
            tableView.reloadData()
        }
    }
}
