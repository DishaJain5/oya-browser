/** Standard OS shortcuts target fixed browser actions and native text editing. */
use super::*;
use objc2::sel;
use objc2_app_kit::{NSMenu, NSMenuItem};
/** Standard native menus provide URL, history, reload, close and text editing shortcuts. */
pub(super) fn menus(app: &NSApplication, target: &Canvas) {
    let menu = NSMenu::new(target.mtm());
    for (title, items) in [
        (
            "Oya Native",
            vec![
                ("Quit Oya Native", "q", chrome::CLOSE),
                ("Clear Saved Cookies…", "", chrome::PROFILE_CLEAR),
                ("Retry Saved Cookies", "", chrome::PROFILE_RETRY),
                ("—", "", chrome::CLOSE),
                ("Clear Saved Site Storage…", "", chrome::STORAGE_CLEAR),
            ],
        ),
        (
            "File",
            vec![
                ("Close", "w", chrome::CLOSE),
                ("Open Location…", "l", chrome::LOCATION),
                ("Reload", "r", chrome::RELOAD),
                ("Back", "[", chrome::BACK),
                ("Forward", "]", chrome::FORWARD),
            ],
        ),
    ] {
        let parent = NSMenuItem::new(target.mtm());
        parent.setTitle(&NSString::from_str(title));
        let submenu =
            NSMenu::initWithTitle(NSMenu::alloc(target.mtm()), &NSString::from_str(title));
        for (label, key, tag) in items {
            if label == "—" {
                submenu.addItem(&NSMenuItem::separatorItem(target.mtm()));
                continue;
            }
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(target.mtm()),
                    &NSString::from_str(label),
                    Some(sel!(activate:)),
                    &NSString::from_str(key),
                )
            };
            item.setTag(tag);
            unsafe {
                item.setTarget(Some(target));
            }
            submenu.addItem(&item);
        }
        parent.setSubmenu(Some(&submenu));
        menu.addItem(&parent);
    }
    let edit = NSMenuItem::new(target.mtm());
    edit.setTitle(&NSString::from_str("Edit"));
    let submenu = NSMenu::new(target.mtm());
    for (label, key, selector) in [
        ("Undo", "z", sel!(undo:)),
        ("Cut", "x", sel!(cut:)),
        ("Copy", "c", sel!(copy:)),
        ("Paste", "v", sel!(paste:)),
        ("Select All", "a", sel!(selectAll:)),
    ] {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(target.mtm()),
                &NSString::from_str(label),
                Some(selector),
                &NSString::from_str(key),
            )
        };
        submenu.addItem(&item);
    }
    edit.setSubmenu(Some(&submenu));
    menu.addItem(&edit);
    app.setMainMenu(Some(&menu));
}
