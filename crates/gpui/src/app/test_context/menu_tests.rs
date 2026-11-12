use crate::{Menu, MenuItem, OsAction, OwnedMenuItem, SystemMenuType, TestAppContext};

crate::actions!(menu_observation_tests, [CopySelection]);

#[crate::test]
fn test_platform_menus_preserve_owned_snapshots_and_replace_contents(cx: &mut TestAppContext) {
    cx.update(|app| {
        assert!(app.get_menus().unwrap().is_empty());
        app.set_menus([
            Menu::new("Edit").items([
                MenuItem::os_action("Copy", CopySelection, OsAction::Copy)
                    .checked(true)
                    .disabled(true),
                MenuItem::separator(),
                MenuItem::submenu(Menu::new("Selection").disabled(true)),
            ]),
            Menu::new("Application")
                .disabled(true)
                .items([MenuItem::os_submenu("Services", SystemMenuType::Services)]),
        ]);

        let mut snapshot = app.get_menus().unwrap();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].name.as_ref(), "Edit");
        assert!(!snapshot[0].disabled);
        let OwnedMenuItem::Action {
            name,
            action,
            os_action,
            checked,
            disabled,
        } = &snapshot[0].items[0]
        else {
            panic!("the first menu item must preserve its action");
        };
        assert_eq!(name, "Copy");
        assert!(action.as_any().is::<CopySelection>());
        assert!(matches!(os_action, Some(OsAction::Copy)));
        assert!(*checked && *disabled);
        assert!(matches!(snapshot[0].items[1], OwnedMenuItem::Separator));
        let OwnedMenuItem::Submenu(submenu) = &snapshot[0].items[2] else {
            panic!("the nested menu must remain a submenu");
        };
        assert_eq!(submenu.name.as_ref(), "Selection");
        assert!(submenu.disabled);
        assert!(snapshot[1].disabled);
        let OwnedMenuItem::SystemMenu(system) = &snapshot[1].items[0] else {
            panic!("the platform menu must preserve its system role");
        };
        assert_eq!(system.name.as_ref(), "Services");
        assert!(matches!(system.menu_type, SystemMenuType::Services));

        snapshot[0].items.clear();
        assert_eq!(app.get_menus().unwrap()[0].items.len(), 3);
        app.set_menus([Menu::new("View")]);
        let replacement = app.get_menus().unwrap();
        assert_eq!(replacement.len(), 1);
        assert_eq!(replacement[0].name.as_ref(), "View");
        assert_eq!(snapshot[0].name.as_ref(), "Edit");
        app.set_menus([]);
        assert!(app.get_menus().unwrap().is_empty());
    });
}
