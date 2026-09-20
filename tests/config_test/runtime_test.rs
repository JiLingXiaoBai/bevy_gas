use bevy::prelude::{App, MinimalPlugins, World};
use bevy_gas::config::generated::gas::{
    Ability, AbilityTask, ActionKind, Attribute, AttributeRegion, DurationKind, Effect,
    MagnitudeKind, Modifier, ModifierOperation, SelectionKind, SortOrder, TargetScope, Targeting,
    TbAbility, TbAbilityTask, TbAttribute, TbEffect, TbModifier, TbTargeting,
};
use bevy_gas::config::generated::{ByteBuf, TABLE_FILES, Tables};
use bevy_gas::config::{
    AbilityId, ConfigErrorKind, ConfigLocation, ConfiguredAbilities, compile_catalog,
    grant_ability, load_tables, read_package, revoke_ability,
};
#[cfg(feature = "config-validation")]
use bevy_gas::config::{
    ConfigError, describe_ability, describe_ability_at_level, package_schema_hash,
    write_package_manifest,
};
use bevy_gas::{AbilitySystemComponent, GameplayAbilitySystemPlugin};
#[cfg(feature = "config-validation")]
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::Arc, time::SystemTime};

pub(super) fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(GameplayAbilitySystemPlugin);
    app
}

pub(super) fn tables(unused_radius: Option<f32>) -> Tables {
    let mut tables = Tables::new(|_| Ok(ByteBuf::new(vec![0]))).unwrap();
    tables.tb_ability = Arc::new(
        TbAbility::from_rows(vec![Ability {
            id: 1,
            name: "Test ability".to_owned(),
            max_level: 2,
            cost_effect_id: None,
            cooldown_effect_id: None,
            asset_tags: vec![],
            required_tags: vec![],
            blocked_tags: vec![],
            cancel_ability_tags: vec![],
            block_ability_tags: vec![],
            targeting_id: 1,
            allow_multiple_instances: false,
        }])
        .unwrap(),
    );
    tables.tb_ability_task = Arc::new(
        TbAbilityTask::from_rows(vec![AbilityTask {
            id: 1,
            ability_id: 1,
            at_tick: 0,
            order: 0,
            kind: ActionKind::EndAbility,
            target_scope: TargetScope::None,
            effect_id: None,
        }])
        .unwrap(),
    );
    tables.tb_targeting = Arc::new(
        TbTargeting::from_rows(vec![Targeting {
            id: 1,
            name: "Self".to_owned(),
            selection: SelectionKind::SelfTarget,
            radius: unused_radius,
            half_angle_radians: None,
            max_distance: None,
            exclude_source: false,
            require_attributes: false,
            required_tags: vec![],
            blocked_tags: vec![],
            sort: SortOrder::None,
            limit: 1,
        }])
        .unwrap(),
    );
    tables
}

#[test]
fn runtime_catalog_grants_and_revokes_without_tooling_apis() {
    let catalog = compile_catalog(&tables(None), app().world_mut()).unwrap();
    assert_eq!(
        catalog.ability(AbilityId(1)).unwrap().name(),
        "Test ability"
    );
    let mut asc = AbilitySystemComponent::default();
    let mut bindings = ConfiguredAbilities::default();
    assert!(grant_ability(&mut asc, &mut bindings, &catalog, AbilityId(1), 0).is_err());
    assert!(bindings.handle(AbilityId(1)).is_none());
    let handle = grant_ability(&mut asc, &mut bindings, &catalog, AbilityId(1), 2).unwrap();
    assert_eq!(bindings.handle(AbilityId(1)), Some(handle));
    assert!(asc.find_ability_spec(handle).is_some());
    assert!(revoke_ability(&mut asc, &mut bindings, AbilityId(1)).unwrap());
    assert!(bindings.handle(AbilityId(1)).is_none());
    assert!(asc.find_ability_spec(handle).is_none());
}

#[test]
fn extra_authoring_validation_depends_on_luban_config_feature() {
    let result = compile_catalog(&tables(Some(1.0)), app().world_mut());
    if cfg!(feature = "config-validation") {
        let error = result.err().unwrap();
        assert_eq!(error.kind(), ConfigErrorKind::Validation);
        assert!(
            matches!(error.location(), ConfigLocation::Table { table: "Targeting", row: Some(row), field: Some(field) } if row == "1" && field == "radius")
        );
    } else {
        assert!(result.unwrap().ability(AbilityId(1)).is_some());
    }
}

#[test]
fn required_references_and_ecs_resources_are_always_checked() {
    let mut tables = tables(None);
    assert!(compile_catalog(&tables, &mut World::new()).is_err());
    tables.tb_targeting = Arc::new(TbTargeting::from_rows(vec![]).unwrap());
    assert!(compile_catalog(&tables, app().world_mut()).is_err());
}

#[test]
fn invalid_effect_numbers_are_always_rejected() {
    for (duration, period, probability) in [
        (None, None, 1.0),
        (Some(0), None, 1.0),
        (Some(-1), None, 1.0),
        (Some(16_777_217), None, 1.0),
        (Some(1), Some(0), 1.0),
        (Some(1), None, f32::NAN),
        (Some(1), None, 1.1),
        (Some(1), None, -0.1),
        (Some(1), None, f32::INFINITY),
    ] {
        let mut tables = tables(None);
        tables.tb_effect =
            Arc::new(TbEffect::from_rows(vec![effect(duration, period, probability)]).unwrap());
        assert!(compile_catalog(&tables, app().world_mut()).is_err());
    }
}

fn effect(duration: Option<i32>, period: Option<i32>, probability: f32) -> Effect {
    Effect {
        id: 1,
        name: "Test effect".to_owned(),
        duration_kind: DurationKind::DurationTicks,
        duration_ticks: duration,
        period_ticks: period,
        execute_on_applied: false,
        probability,
        asset_tags: vec![],
        granted_tags: vec![],
    }
}

#[test]
fn invalid_levels_and_unrepresentable_targeting_distances_are_always_rejected() {
    let mut invalid_level = tables(None);
    let mut ability = invalid_level.tb_ability.get(&1).unwrap().as_ref().clone();
    ability.max_level = 0;
    invalid_level.tb_ability = Arc::new(TbAbility::from_rows(vec![ability]).unwrap());
    assert!(compile_catalog(&invalid_level, app().world_mut()).is_err());

    for (selection, radius, max_distance) in [
        (SelectionKind::Sphere, Some(f32::MAX), None),
        (SelectionKind::SelfTarget, None, Some(f32::MAX)),
    ] {
        let mut tables = tables(None);
        let mut targeting = tables.tb_targeting.get(&1).unwrap().as_ref().clone();
        targeting.selection = selection;
        targeting.radius = radius;
        targeting.max_distance = max_distance;
        tables.tb_targeting = Arc::new(TbTargeting::from_rows(vec![targeting]).unwrap());
        assert!(compile_catalog(&tables, app().world_mut()).is_err());
    }
}

#[test]
fn modifier_parameter_checks_respect_magnitude_kind_and_feature() {
    let unused_slope_is_allowed = !cfg!(feature = "config-validation");
    for (magnitude_kind, base, per_level, valid) in [
        (MagnitudeKind::LinearLevel, 1.0, 1.0, true),
        (MagnitudeKind::LinearLevel, f32::NAN, 1.0, false),
        (MagnitudeKind::LinearLevel, 1.0, f32::INFINITY, false),
        (MagnitudeKind::Flat, 1.0, f32::NAN, unused_slope_is_allowed),
        (
            MagnitudeKind::Flat,
            1.0,
            f32::INFINITY,
            unused_slope_is_allowed,
        ),
    ] {
        let mut tables = tables(None);
        tables.tb_attribute = Arc::new(
            TbAttribute::from_rows(vec![Attribute {
                name: "Health".to_owned(),
                region: AttributeRegion::Hot,
                description: String::new(),
            }])
            .unwrap(),
        );
        tables.tb_effect = Arc::new(TbEffect::from_rows(vec![effect(Some(1), None, 1.0)]).unwrap());
        tables.tb_modifier = Arc::new(
            TbModifier::from_rows(vec![Modifier {
                id: 1,
                effect_id: 1,
                order: 0,
                attribute: "Health".to_owned(),
                operation: ModifierOperation::Add,
                magnitude_kind,
                base,
                per_level,
            }])
            .unwrap(),
        );
        assert_eq!(compile_catalog(&tables, app().world_mut()).is_ok(), valid);
    }
}

#[test]
fn probability_endpoints_are_supported() {
    for probability in [0.0, 1.0] {
        let mut tables = tables(None);
        tables.tb_effect =
            Arc::new(TbEffect::from_rows(vec![effect(Some(1), None, probability)]).unwrap());
        let catalog = compile_catalog(&tables, app().world_mut()).unwrap();
        assert_eq!(catalog.effect_count(), 1);
    }
}

#[cfg(feature = "config-validation")]
#[test]
fn public_inspection_apis_preserve_default_level_and_errors() {
    let tables = tables(None);
    let default_report = describe_ability(&tables, AbilityId(1)).unwrap();
    assert_eq!(
        default_report,
        describe_ability_at_level(&tables, AbilityId(1), 1).unwrap()
    );
    assert!(default_report.contains("Test ability | level 1/2"));
    assert!(
        describe_ability_at_level(&tables, AbilityId(1), 2)
            .unwrap()
            .contains("level 2/2")
    );
    let error: ConfigError = describe_ability_at_level(&tables, AbilityId(1), 3).unwrap_err();
    assert_eq!(error.kind(), ConfigErrorKind::UnsupportedLevel);
    assert!(
        matches!(error.location(), ConfigLocation::Table { table: "Ability", row: Some(row), field: Some(field) } if row == "1" && field == "level")
    );
    assert!(describe_ability(&tables, AbilityId(99)).is_err());
}

struct Package(PathBuf);

impl Package {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("bevy-gas-config-{}-{nonce}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let package = Self(directory);
        for name in TABLE_FILES {
            fs::write(package.0.join(format!("{name}.bytes")), [0_u8]).unwrap();
        }
        package
    }

    #[cfg(feature = "config-validation")]
    fn write_manifest(&self, manifest: &Value) {
        fs::write(
            self.0.join("manifest.json"),
            serde_json::to_vec(manifest).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn package_loading_and_required_table_checks_are_always_available() {
    let package = Package::new();
    #[cfg(feature = "config-validation")]
    write_package_manifest(&package.0).unwrap();

    assert_eq!(read_package(&package.0).unwrap().len(), TABLE_FILES.len());
    let tables = load_tables(&package.0).unwrap();
    let catalog = compile_catalog(&tables, app().world_mut()).unwrap();
    assert_eq!(catalog.abilities().count(), 0);

    let table_path = package.0.join(format!("{}.bytes", TABLE_FILES[0]));
    fs::write(&table_path, []).unwrap();
    #[cfg(feature = "config-validation")]
    write_package_manifest(&package.0).unwrap();
    let error = load_tables(&package.0).unwrap_err();
    assert_eq!(error.kind(), ConfigErrorKind::Decode);
    assert!(
        matches!(error.location(), ConfigLocation::File { path, byte_offset: Some(0), .. } if path == &table_path)
    );

    fs::remove_file(&table_path).unwrap();
    let error = read_package(&package.0).unwrap_err();
    assert_eq!(error.kind(), ConfigErrorKind::Io);
    assert!(matches!(error.location(), ConfigLocation::File { path, .. } if path == &table_path));
    assert!(load_tables(&package.0).is_err());
}

#[cfg(not(feature = "config-validation"))]
#[test]
fn runtime_package_loading_ignores_missing_and_invalid_manifests() {
    let package = Package::new();
    assert!(!package.0.join("manifest.json").exists());
    assert_eq!(read_package(&package.0).unwrap().len(), TABLE_FILES.len());
    assert!(load_tables(&package.0).is_ok());

    fs::write(package.0.join("manifest.json"), b"not valid JSON").unwrap();
    assert_eq!(read_package(&package.0).unwrap().len(), TABLE_FILES.len());
    assert!(load_tables(&package.0).is_ok());
}

#[cfg(feature = "config-validation")]
#[test]
fn enabled_validation_checks_manifest_schema_and_hashes() {
    let package = Package::new();
    assert!(load_tables(&package.0).is_err());
    write_package_manifest(&package.0).unwrap();
    assert!(load_tables(&package.0).is_ok());

    let table_path = package.0.join(format!("{}.bytes", TABLE_FILES[0]));
    fs::write(&table_path, [1_u8]).unwrap();
    let error = load_tables(&package.0).unwrap_err();
    assert_eq!(error.kind(), ConfigErrorKind::Package);
    assert!(
        matches!(error.location(), ConfigLocation::File { path, field: Some(field), .. } if path == &table_path && field == "hash")
    );
    fs::write(&table_path, [0_u8]).unwrap();

    let mut manifest: Value =
        serde_json::from_slice(&fs::read(package.0.join("manifest.json")).unwrap()).unwrap();
    manifest["schema_hash"] = json!("incompatible-schema");
    package.write_manifest(&manifest);
    let error = load_tables(&package.0).unwrap_err();
    assert_eq!(error.kind(), ConfigErrorKind::Package);
    assert!(
        matches!(error.location(), ConfigLocation::File { path, field: Some(field), .. } if path == &package.0.join("manifest.json") && field == "schema_hash")
    );
    manifest["schema_hash"] = json!(package_schema_hash());
    manifest["files"].as_array_mut().unwrap().pop();
    package.write_manifest(&manifest);
    assert!(load_tables(&package.0).is_err());
}

#[test]
fn generated_decoding_rejects_truncated_and_trailing_bytes() {
    for malformed in [vec![], vec![1], vec![0, 0]] {
        let result = Tables::new(|name| {
            Ok(ByteBuf::new(if name == TABLE_FILES[0] {
                malformed.clone()
            } else {
                vec![0]
            }))
        });
        assert!(result.is_err());
    }
}
