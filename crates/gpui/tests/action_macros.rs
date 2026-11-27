use open_gpui::{Action, actions};
use open_gpui_macros::register_action;
use serde::Deserialize;
use serde_json::json;

#[test]
fn test_action_macros() {
    actions!(
        test_only,
        [
            SomeAction,
            /// Documented action
            SomeActionWithDocs,
        ]
    );

    #[derive(PartialEq, Clone, Deserialize, Action)]
    #[action(namespace = test_only)]
    #[serde(deny_unknown_fields)]
    struct AnotherAction;

    #[derive(PartialEq, Clone, open_gpui::private::serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RegisterableAction {}

    register_action!(RegisterableAction);

    impl open_gpui::Action for RegisterableAction {
        fn boxed_clone(&self) -> Box<dyn open_gpui::Action> {
            unimplemented!()
        }

        fn partial_eq(&self, _action: &dyn open_gpui::Action) -> bool {
            unimplemented!()
        }

        fn name(&self) -> &'static str {
            unimplemented!()
        }

        fn name_for_type() -> &'static str
        where
            Self: Sized,
        {
            "test_only::RegisterableAction"
        }

        fn build(_value: serde_json::Value) -> anyhow::Result<Box<dyn open_gpui::Action>>
        where
            Self: Sized,
        {
            unimplemented!()
        }
    }
}

#[test]
fn parameterized_action_builds_from_json() -> anyhow::Result<()> {
    #[derive(Debug, PartialEq, Clone, Deserialize, Action)]
    #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
    #[action(namespace = test_only)]
    struct ParameterizedAction {
        count: u32,
    }

    let data = open_gpui::generate_list_of_all_registered_actions()
        .find(|data| data.name == "test_only::ParameterizedAction")
        .expect("the action is registered under its qualified name");
    let action = (data.build)(json!({"count": 7}))?;
    assert_eq!(
        action.as_any().downcast_ref::<ParameterizedAction>(),
        Some(&ParameterizedAction { count: 7 })
    );
    assert!((data.build)(json!({"count": "seven"})).is_err());

    #[cfg(feature = "schemars")]
    {
        let schema = (data.json_schema)(&mut schemars::SchemaGenerator::default())
            .expect("a parameterized action has a schema");
        assert_eq!(schema.as_value()["properties"]["count"]["type"], "integer");
    }

    Ok(())
}
