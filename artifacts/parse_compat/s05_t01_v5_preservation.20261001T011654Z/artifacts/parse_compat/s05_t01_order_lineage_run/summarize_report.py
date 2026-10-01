import json

p = "artifacts/parse_compat/native_body_order_lineage.json"
d = json.load(open(p))
print("schema", d["schema_version"], "| lane", d["lane"], "| thresholds", d["thresholds"])
print("order_axes:", [(a["lane"], a["disposition_label"], a["promoted"]) for a in d["order_axes"]])
for doc in d["documents"]:
    print("=", doc["document_id"], "pages", doc["pages"], "spans", doc["pdf_inspector_spans"])
    audit = doc["order_audit"]
    if audit:
        for ln in audit["lanes"]:
            print("   lane", ln["lane"], "n", ln["token_count"], "order", ln["order_digest"][:20], "identity", ln["identity_digest"][:20])
        for pr in audit["pairs"]:
            print("   pair", pr["left_lane"], "vs", pr["right_lane"], "positions_differing", pr["positions_differing"], "identical", pr["identical_order"])
    else:
        print("   audit absent:", doc["order_audit_absence"])
    for o in doc["orders"]:
        loc = o["localization"]
        print("   axis", o["lane"], o["status"], "tokens", o["token_count"], "plane_units", o["plane_units"],
              "exact", loc["exact_in_plane"] if loc else None,
              "boundary", loc["boundary_variant_from_plane"] if loc else None,
              "uncertain", (loc["uncertain_present_in_token_from_plane"] + loc["uncertain_spanning_tokens_from_plane"] + loc["uncertain_character_material_from_plane"]) if loc else None,
              "absent", loc["absent_from_plane"] if loc else None,
              "verdict", o["verdict"]["verdict"] if o["verdict"] else None)
    print("   p2 s03", doc["p2_primary_lane"]["s03_parent_span"]["status"], doc["p2_primary_lane"]["s03_parent_span"]["token_count"])
    print("   p2 word", doc["p2_primary_lane"]["word_heading"]["status"], doc["p2_primary_lane"]["word_heading"]["token_count"])
print("aggregate order axes:")
for a in d["aggregate"]["order_axes"]:
    loc = a["localization"]
    print("  ", a["lane"], "measured", a["documents_measured"], "/", a["documents_declared"],
          "oracle_units", loc["oracle_units"], "exact", loc["exact_in_plane"], "uncertain", loc["uncertain_present_in_token_from_plane"] + loc["uncertain_spanning_tokens_from_plane"] + loc["uncertain_character_material_from_plane"],
          "absent", loc["absent_from_plane"], "verdict", a["verdict"]["verdict"], "d498", a["verdict"]["d498_reopen_required"])
print("prior artifacts:", [(b["path"].split("/")[-1], (b["sha256"] or "ABSENT")[:16]) for b in d["prior_artifacts"]])
