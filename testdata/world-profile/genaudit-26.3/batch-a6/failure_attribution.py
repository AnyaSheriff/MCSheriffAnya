#!/usr/bin/env python3
"""Классификация 241 исторического провала без выдуманной атрибуции отдельных предикатов."""
import json
from pathlib import Path
from collections import Counter
root=Path.cwd();m=json.loads((root/'testdata/world-profile/genaudit-26.3/batch-a6/measurements.json').read_text());rows=[]
for label,s in m['seeds'].items():
 for r in s['new_outside_common']:
  metric=r['metric'];batch=r['batch'];kind=metric.split('/')[0]
  if batch=='A':
   if kind in ['height_biome','height_mean','height','mountain_local_peaks_ge128']:cause='terrain_height_distribution'
   elif kind in ['layer','top']:cause='surface_exposure_and_legacy_elevation_cutoff'
   elif kind=='surface_band':cause='palette_exposure_at_changed_heights'
   elif metric=='blocks/bubble_column':cause='water_surface_and_column_availability'
   elif metric=='blocks/red_sand':cause='low_badlands_surface_area'
   elif kind=='feature_biome':cause='substrate_and_biome_volume'
   else:raise ValueError(r)
  elif batch=='U':
   if kind=='depth':cause='terrain_and_aquifer_volume'
   elif kind in ['ore','blocks']:cause='ore_host_volume_at_height'
   elif kind=='feature_biome':cause='cave_biome_volume_and_feature_space'
   else:raise ValueError(r)
  elif batch in ['F-tree','F','F-ocean']:cause='feature_substrate_slope_water_and_biome_volume'
  else:raise ValueError(r)
  proof='Unchanged feature code and RNG; density-only first-prototype counterfactual. Mechanism class; individual predicate attribution is not demonstrated.'
  bug=False
  if metric=='layer/cherry_grove/-3/dirt' and label=='second':
   bug=True;proof='Confirmed legacy integration defect: fuzzy neighbouring ordinary biome above Y112 (sunflower_plains in paired witnesses) loses soil from its height alone. Generated high-forest regression and paired native cherry columns recorded.'
  rows.append({'seed':label,'batch':batch,'metric':metric,'cause_class':cause,'confirmed_bug_exposure':bug,'evidence':proof,'before_default':r['before'],'first_prototype':r['ours'],'vanilla':r['vanilla']})
result={'total':len(rows),'by_batch':dict(Counter(r['batch'] for r in rows)),'by_primary_mechanism':dict(Counter(r['cause_class'] for r in rows)),'interpretation':'All 241 are downstream effects of the density-only first prototype. This does not establish 241 feature algorithm bugs. One of these rows is directly tied to a confirmed legacy surface integration defect. The separately confirmed jagged-peaks carving integration defect is absent from the A3 footprint and is not counted among these 241. Remaining surface rows mix geometric exposure with the legacy cutoff; per-row causal fractions require a controlled ablation and remain unverified. No independent tree/plant/ocean placement algorithm bug was demonstrated in this round.','confirmed_integration_defects':[{'defect':'Caves are carved after terrain without a density-aware upper-body surface guard','evidence':'Jagged-peaks raw terrain Y144 air 0/29496 before carving; native final air 4963/29496. Exact raw geometry denominator matches the native profile.','in_241':False},{'defect':'Ordinary surface rules remove grass/dirt solely above Y112, including fuzzy ordinary-biome neighbours of cherry grove','in_241_confirmed_rows':1,'potential_surface_rows':87}],'rows':rows}
assert result['total']==241
(root/'testdata/world-profile/genaudit-26.3/batch-a6/calibration/failure-attribution.json').write_text(json.dumps(result,indent=2)+'\n')
print(result['by_primary_mechanism'])
