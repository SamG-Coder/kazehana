"""Probe: what the MetaHuman Python API offers in this engine (run headless in D:/KazehanaHumans)."""
import unreal
out = []
for name in ["MetaHumanCharacter", "MetaHumanCharacterEditorSubsystem", "MetaHumanCharacterExportBlueprintLibrary",
             "MetaHumanGeometryExportParams", "MetaHumanDCCExportParams", "MetaHumanCharacterBodyConstraint"]:
    c = getattr(unreal, name, None)
    out.append(f"{name}: {'yes' if c else 'MISSING'}")
    if c:
        out.append("   " + ", ".join(sorted(m for m in dir(c) if not m.startswith("_")))[:3000])
# presets shipped with the plugin
reg = unreal.AssetRegistryHelpers.get_asset_registry()
for a in reg.get_assets_by_path("/MetaHumanCharacter", recursive=True):
    if "MetaHumanCharacter" == str(a.asset_class_path.asset_name):
        out.append(f"preset: {a.package_name}")
open("D:/KazehanaHumans/probe.txt", "w").write("\n".join(out))
