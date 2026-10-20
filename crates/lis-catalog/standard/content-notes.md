# Standard Catalog 1.1.0: clinical cutoff evidence

Clinical cutoff evidence checked 2026-10-14; menu, terminology and unit evidence checked 2026-10-13. These notes describe the shipped US clinical toxicology starter content for REQ-OPS-014 and REQ-OPS-015. Its `reviewStatus` is `unreviewed`: source transcription and software checks do not supply laboratory approval or clinical validation. Source IDs, publishers, dates and limits are maintained in the [domain source ledger](../../../docs/research/sources-domain.md#standard-catalog-content-sources).

The [shipped JSON](catalog-v1.json) holds **101 sourced drug cutoffs out of 106 drug entries**, with **5 unsupported drug cutoffs and 5 specimen-validity fields blank**. All populated cutoffs are ng/mL lexical decimals. The file contains no patient data. Values are transcribed without inferred conversions or averaging.

## Selection and limits

The sources demonstrate published clinical practice, not a representative survey of cutoff prevalence. Therefore these are **selected published starting values**, not a claim that each is the nationally most common cutoff. Where laboratories agree, the comparison records that agreement; disagreements and incompatible methods remain visible. A source documenting one assay does not validate another laboratory's assay.

| Area | Selection rule and conditions |
| --- | --- |
| Urine screens | Prefer clinical directory examples; compare Mayo and ARUP, with Quest and Michigan MLabs where applicable. Roche supplies a named urine 6-AM immunoassay example. A class cutoff is calibrated to its assay target, not equal sensitivity to every class member. |
| Urine definitive opiates | Use Mayo's 25 ng/mL **total** (unconjugated + conjugated) examples consistently; 6-AM uses ARUP's 10 ng/mL **free** example. ARUP's 20 ng/mL free-opiate alternatives are not interchangeable with total-drug methods. |
| Other urine definitive tests | Use analyte-specific clinical LC-MS/MS directories, usually ARUP; fill uncovered analytes from Mayo, Ohio State and Saint Francis. Buprenorphine/norbuprenorphine examples are unconjugated, with glucuronides measured separately by the source. Resolve hydrolysis/preparation where the directory does not state it. |
| Oral-fluid definitive tests | Prefer Quest's clinical Quantisal examples (November 2022 brochure); Labcorp MedTox corroborates many class thresholds and supplies explicit 6-AM. Class-level corroboration is not evidence for an unlisted metabolite. |
| Oral-fluid screens | Mayo's clinical Quantisal immunoassays with named targets; THC uses the specific LZI prescription enzyme assay, and cocaine/BZE the specific OralTox prescription lateral-flow assay. These are examples from different assays/collectors, not one validated combined panel. |
| Oral-fluid concentrations | Copy published numbers without multiplying/dividing for buffer. Quest, Mayo and MedTox do not explicitly resolve neat-versus-buffered reporting basis in the cited pages; local collection and reporting basis must be matched. Only the cited LZI THC example explicitly states neat oral fluid. |
| Other limits and rules | Do not populate LOD, LOQ, reportable ranges, validity intervals, or medical interpretations from these cutoffs. Keep threshold direction and rounding as starter values to confirm. |

## Per-analyte comparisons

Values below are ng/mL except where an alternative explicitly includes its original unit. Parent drug, metabolite, screen and definitive measurements remain separate. The selected source is the one embedded in the file and shown in the preview; alternatives are evidence for review, not extra active cutoffs. Source publisher, dates and applicability are recorded in the [domain source ledger](../../../docs/research/sources-domain.md#clinical-starter-cutoff-sources-2026-10-14).

### Urine

| Analyte code | Selected cutoff | Selected source | Other published examples / qualifications |
| --- | ---: | --- | --- |
| `UR-AMP-IA` | 500 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 300; [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 500 |
| `UR-BAR-IA` | 200 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 200; [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 300 |
| `UR-BZO-IA` | 100 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 200; [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 100 |
| `UR-BUP-IA` | 5 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | Single selected assay example; prevalence not established. |
| `UR-THC-IA` | 50 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 50 |
| `UR-COC-IA` | 150 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 150; [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 150 |
| `UR-MTD-IA` | 150 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | Single selected assay example; prevalence not established. |
| `UR-EDDP-IA` | 100 | [DOM-145](https://mlabs.umich.edu/tests/methadone-metabolite-eddp-urine-qualitative-screen) | [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 100; [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410): 300 |
| `UR-FEN-IA` | 1 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | Single selected assay example; prevalence not established. |
| `UR-OPI-IA` | 300 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 300 |
| `UR-OXY-IA` | 100 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 100 |
| `UR-6AM-IA` | 10 | [DOM-146](https://diagnostics.roche.com/global/en/products/lab/6am-cps-000001.html) | Single selected assay example; prevalence not established. |
| `UR-PCP-IA` | 25 | [DOM-143](https://www.mayocliniclabs.com/test-catalog/overview/81410) | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312): 25; [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 25 |
| `UR-TRA-IA` | 100 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | Single selected assay example; prevalence not established. |
| `UR-MDMA-IA` | 500 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | Single selected assay example; prevalence not established. |
| `UR-ETG-IA` | 500 | [DOM-142](https://ltd.aruplab.com/Tests/Pub/2012312) | Single selected assay example; prevalence not established. |
| `UR-MOR` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-COD` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-6AM` | 10 | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 10 |
| `UR-DHC` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | Single selected assay example; prevalence not established. |
| `UR-HYC` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-NHYC` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-HYM` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-OXYC` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-NOXYC` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-OXYM` | 25 | [DOM-148](https://www.mayocliniclabs.com/test-catalog/overview/8473) | [DOM-147](https://ltd.aruplab.com/Tests/Pub/0090364): 20 (free); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-FENT` | 1 | [DOM-150](https://ltd.aruplab.com/Tests/Pub/0092570) | [DOM-151](https://www.mayocliniclabs.com/test-catalog/overview/89655): .2; [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 2 |
| `UR-NFENT` | 2 | [DOM-150](https://ltd.aruplab.com/Tests/Pub/0092570) | [DOM-151](https://www.mayocliniclabs.com/test-catalog/overview/89655): 1; [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 10 |
| `UR-MTD` | 100 | [DOM-152](https://ltd.aruplab.com/Tests/Pub/0090362) | [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 100 (MS; chromatography not verified); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-EDDP` | 100 | [DOM-152](https://ltd.aruplab.com/Tests/Pub/0090362) | [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 100 (MS; chromatography not verified); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-BUP` | 2 | [DOM-153](https://ltd.aruplab.com/Tests/Pub/2010092) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 5 |
| `UR-NBUP` | 2 | [DOM-153](https://ltd.aruplab.com/Tests/Pub/2010092) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 20 |
| `UR-TRA` | 25 | [DOM-154](https://ltd.aruplab.com/Tests/Pub/2002736) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-ODT` | 25 | [DOM-154](https://ltd.aruplab.com/Tests/Pub/2002736) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-TAP` | 50 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | Single selected assay example; prevalence not established. |
| `UR-NTAP` | 50 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | Single selected assay example; prevalence not established. |
| `UR-ALP` | 5 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-AOHALP` | 5 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-CLO` | 5 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-7ACLO` | 5 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-DIAZ` | 20 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-NORD` | 20 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-OXAZ` | 20 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-TEMA` | 20 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-LORA` | 20 | [DOM-155](https://ltd.aruplab.com/Tests/Pub/2008291) | [DOM-156](https://www.mayocliniclabs.com/test-catalog/overview/608255): 10 (hydrolysis); [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-AMP` | 50 | [DOM-157](https://ltd.aruplab.com/Tests/Pub/2010075) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100; [DOM-158](https://www.mayocliniclabs.com/test-catalog/overview/8257): 25 |
| `UR-MAMP` | 200 | [DOM-157](https://ltd.aruplab.com/Tests/Pub/2010075) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100; [DOM-158](https://www.mayocliniclabs.com/test-catalog/overview/8257): 25 |
| `UR-MDMA` | 200 | [DOM-157](https://ltd.aruplab.com/Tests/Pub/2010075) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100; [DOM-158](https://www.mayocliniclabs.com/test-catalog/overview/8257): 25 |
| `UR-MDA` | 200 | [DOM-157](https://ltd.aruplab.com/Tests/Pub/2010075) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100; [DOM-158](https://www.mayocliniclabs.com/test-catalog/overview/8257): 25 |
| `UR-MDEA` | 200 | [DOM-157](https://ltd.aruplab.com/Tests/Pub/2010075) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100 |
| `UR-BZE` | 50 | [DOM-159](https://ltd.aruplab.com/Tests/Pub/0090359) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50; [DOM-160](https://theohiostatelabs.testcatalog.org/show/COCAMS): 25 |
| `UR-COC` | 25 | [DOM-160](https://theohiostatelabs.testcatalog.org/show/COCAMS) | Single selected assay example; prevalence not established. |
| `UR-THCA` | 15 | [DOM-161](https://ltd.aruplab.com/Tests/Pub/0090369) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 25 (THC glucuronide not tested) |
| `UR-BUTA` | 50 | [DOM-162](https://ltd.aruplab.com/Tests/Pub/2012213) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100 |
| `UR-PHENO` | 50 | [DOM-162](https://ltd.aruplab.com/Tests/Pub/2012213) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100 |
| `UR-PENTO` | 50 | [DOM-162](https://ltd.aruplab.com/Tests/Pub/2012213) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 100 (pentobarbital/amobarbital combined) |
| `UR-SECO` | 100 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | [DOM-144](https://testdirectory.questdiagnostics.com/test/test-detail/91486/?cc=NEL): 100 (MS; chromatography not verified) |
| `UR-PCP` | 25 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | Single selected assay example; prevalence not established. |
| `UR-KET` | 50 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | Single selected assay example; prevalence not established. |
| `UR-ETG` | 250 | [DOM-163](https://www.testcatalog.org/show/ETGC) | Single selected assay example; prevalence not established. |
| `UR-ETS` | 100 | [DOM-163](https://www.testcatalog.org/show/ETGC) | Single selected assay example; prevalence not established. |
| `UR-CARI` | 100 | [DOM-165](https://ltd.aruplab.com/Tests/Pub/2012219) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-MEPRO` | 100 | [DOM-165](https://ltd.aruplab.com/Tests/Pub/2012219) | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms): 50 |
| `UR-GABA` | 500 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | [DOM-166](https://ltd.aruplab.com/Tests/Pub/2012227): 5000 (source: 5.0 ug/mL) |
| `UR-PREG` | 500 | [DOM-149](https://www.saintfrancis.com/physicians/laboratory-testing/biochemical-genetics-mass-spectrometry-tests/clinical-urine-drug-screen-reflex-confirmation-lcmsms) | [DOM-167](https://ltd.aruplab.com/Tests/Pub/2012229): 5000 (source: 5.0 ug/mL) |

### Oral fluid

| Analyte code | Selected cutoff | Selected source | Other published examples / qualifications |
| --- | ---: | --- | --- |
| `OF-AMP-IA` | 20 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-THC-IA` | 4 | [DOM-171](https://www.accessdata.fda.gov/cdrh_docs/reviews/K141320.pdf) | Single selected assay example; prevalence not established. |
| `OF-COC-IA` | 20 | [DOM-172](https://www.accessdata.fda.gov/cdrh_docs/reviews/K171403.pdf) | Single selected assay example; prevalence not established. |
| `OF-MTD-IA` | 10 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-FEN-IA` | 1 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-OPI-IA` | 10 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-OXY-IA` | 10 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-6AM-IA` | 5 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-PCP-IA` | 5 | [DOM-170](https://www.mayocliniclabs.com/test-catalog/Overview/621483) | Single selected assay example; prevalence not established. |
| `OF-MOR` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-COD` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-6AM` | 1 | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid) | Single selected assay example; prevalence not established. |
| `OF-HYC` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-HYM` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-OXYC` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-OXYM` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-FENT` | 0.1 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.1 |
| `OF-MTD` | 5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | Single selected assay example; prevalence not established. |
| `OF-EDDP` | 5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | Single selected assay example; prevalence not established. |
| `OF-BUP` | 0.1 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 1 |
| `OF-NBUP` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 1 |
| `OF-TRA` | 5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 5 |
| `OF-TAP` | 5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 5 |
| `OF-ALP` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.5 |
| `OF-CLO` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.5 |
| `OF-DIAZ` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.5 |
| `OF-NORD` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.5 |
| `OF-OXAZ` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.5 |
| `OF-TEMA` | 0.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 0.5 |
| `OF-AMP` | 10 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 10 |
| `OF-MAMP` | 10 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 10 |
| `OF-MDMA` | 10 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | Single selected assay example; prevalence not established. |
| `OF-COC` | 5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-BZE` | 5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-THC` | 2.5 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 2.5 |
| `OF-PCP` | 10 | [DOM-168](https://www.questdiagnostics.com/content/dam/corporate/unrestricted/documents/oral-fluid-collection/Oral-Fluid-Collection-Brochure_FINAL.pdf) | [DOM-169](https://medtox.labcorp.com/tests/701687/drug-analysis-comp-oral-fluid): 10 |

## Remaining blanks

| Analyte code | Reason |
| --- | --- |
| `UR-CREAT` | Specimen-validity measurement requires laboratory-approved reference/hold policy, often intervals, two-sided limits or combined rules; single at-or-above positive-drug cutoff is not appropriate. |
| `UR-SG` | Specimen-validity measurement requires laboratory-approved reference/hold policy, often intervals, two-sided limits or combined rules; single at-or-above positive-drug cutoff is not appropriate. |
| `UR-PH` | Specimen-validity measurement requires laboratory-approved reference/hold policy, often intervals, two-sided limits or combined rules; single at-or-above positive-drug cutoff is not appropriate. |
| `UR-OXID` | Specimen-validity measurement requires laboratory-approved reference/hold policy, often intervals, two-sided limits or combined rules; single at-or-above positive-drug cutoff is not appropriate. |
| `UR-NITRITE` | Specimen-validity measurement requires laboratory-approved reference/hold policy, often intervals, two-sided limits or combined rules; single at-or-above positive-drug cutoff is not appropriate. |
| `UR-AMO` | Saint Francis publishes combined amobarbital/pentobarbital, not individual amobarbital; Quest publishes individual 100 ng/mL but exact chromatography technique not verified on its directory page. No inferred individual LC-MS/MS cutoff. |
| `OF-MDMA-IA` | No current clinical MDMA-specific oral-fluid immunoassay cutoff verified. Quest MDMA 10 is LC-MS/MS, not immunoassay; prescription methamphetamine-assay cross-reactivity is not an independently calibrated MDMA screen. |
| `OF-NFENT` | No sufficiently specific current public clinical directory cutoff verified. Historical Precision Diagnostics clinical LC-MS/MS brochure MRK-1166-1-2003 lists 0.2 ng/mL, but direct URL now returns 404 and collector/reporting dilution basis is absent. Cached document establishes a historical lead, not a suitable new default. |
| `OF-7ACLO` | No individual quantitative current clinical directory cutoff verified. HNL publishes only a qualitative class threshold. Vision Laboratories synthetic LC-MS/MS report printed 2019-04-04, hosted by third-party 4UHealth, lists 1 ng/mL but lacks collection/dilution basis and current corroboration; not adopted. |
| `OF-MDA` | No specific current clinical directory cutoff verified. Quest supplies MDMA, not MDA; Labcorp class cutoff does not explicitly establish MDA. Vision Laboratories synthetic LC-MS/MS report printed 2019-04-04 lists 5 ng/mL on third-party 4UHealth, but lacks collection/dilution basis and current corroboration; not adopted. |

## Applying this content

A load uses content version 1.1.0 and creates drafts. The file remains unreviewed; the loading warning and laboratory review requirements apply. The [format contract](README.md) defines the shipped records.

**Existing installations:** loading again skips methods already present; updating the application does not replace saved cutoffs. The draft's author can edit it in Configuration → Methods. For an approved method, create a new method draft and new versions of affected orderables, generate a new Catalog Preview, and approve that exact reviewed set under the configured separation-of-duties policy. Fresh confirmation and any required self-approval acknowledgment still apply ([ADR 0015](../../../docs/architecture/decisions/0015-separation-of-duties-policy.md), [ADR 0018](../../../docs/architecture/decisions/0018-remediation-authority-and-method-continuity.md)). This preserves earlier method versions and frozen order manifests. A refreshed preview is required after any edits.

**Preview labels compare current values, not historical origin.** Source matching uses code and value against the file in the current build. A saved or demo value may differ from the current starter or happen to equal it. The preview says “Differs from current starter value” and explains that comparison; a match is not proof of the saved value's provenance. Retained preview artifacts and their template versions remain immutable.

**Demo values:** the separate demo setup fills only the five unsupported drug-cutoff gaps with explicitly demo-only values. It never substitutes for a published cutoff or supplies specimen-validity rules. Saved values are not replaced by an application update and still require review.

## Menu and laboratory review

| Shipped content | Count | Scope |
| --- | --- | --- |
| Specimen types | 2 | `UR` urine and `OF` oral fluid |
| Drug classes | 14 | Project pharmacological groups; not a CPT class list or billing taxonomy |
| Analytes | 111 | Urine: 16 screens, 50 definitive, 5 validity. Oral fluid: 10 screens, 30 definitive |
| Methods | 27 | Two screen methods, one urine-validity method and 24 class/specimen LC-MS/MS methods |
| Orderables | 27 | Twenty-four definitive tests, a urine screen panel, an oral-fluid screen test and a separate urine-validity test |

These counts describe the shipped JSON, not a recommended menu or laboratory capacity. Confirm the laboratory's menu, class grouping, screen calibrators, preparation, collection basis, units, threshold direction and rounding before approving drafts. Source qualifications do not replace an approved method procedure.

| Content choice | Provenance and remaining review |
| --- | --- |
| Specimen codes | `UR` follows HL7 v2 Table 0487 (DOM-122). `OF` is a project code for oral fluid, distinct from Table 0487's `SAL` saliva. Any future `OF` → `SAL` interface mapping needs explicit qualification against specimen meaning; DOM-118 is workplace-context evidence, not a clinical cutoff source. |
| Pharmacological grouping | Opiates include oxycodone; tramadol/tapentadol, gabapentin/pregabalin and PCP/ketamine share respective project groups. The laboratory must confirm these groupings. They confer no payer class mapping or order authority (REG-031, INV-13). |
| Screening and definitive records | Screen targets are class-level analytes, distinct from specific molecules. Each definitive class/specimen method has its own test; the catalog supplies no blanket presumptive-plus-definitive bundle or billing-tier panel. The urine screen includes a separate validity component, which is also independently orderable. |
| Validity method | The shipped `presumptive`/`quantitative` settings require laboratory confirmation. DOM-117 §3.1 describes workplace initial-validity testing; its thresholds are not adopted. All five validity cutoff fields are blank. |
| Units | The retained NLM UCUM syntax check (DOM-130, 2026-10-13) covers `ng/mL`, `mg/dL`, `ug/mL`, `1` and `[pH]`; syntax is not assay validation. Confirm oxidant/nitrite `ug/mL` meaning or a separately specified qualitative result. |
| Threshold semantics | Every method ships `at_or_above` and `before_rounding` as values to confirm. Source examples support at-or-above reporting (DOM-120 and workplace-context DOM-117/118); the evidence review found no rounding-order authority. A qualitative screen's concentration unit does not make its result quantitative. |
| EtG screen | DOM-120 documents a laboratory-developed urine immunoassay profile. The 2026-10-13 source review did not establish an FDA-cleared EtG assay; the starter record does not confer clearance or validate the local assay. |

### Analyte evidence

The following sources support analyte inclusion and specimen context. Their detailed applicability and reuse terms remain in the [source ledger](../../../docs/research/sources-domain.md#standard-catalog-content-sources) and shipped source records. They do not supply the clinical cutoffs unless the per-analyte comparison explicitly identifies them as the selected cutoff source.

| Evidence | Contribution and boundary |
| --- | --- |
| HHS panels, DOM-115 | Federal-panel analyte names in urine/oral fluid. Workplace program evidence only; none of the current clinical cutoff fields cites this source. |
| ADLM pain-management guideline, DOM-024 | Drug/metabolite menu examples and specimen discussion, including urine parent drugs and oral-fluid examples. Laboratory review must establish intended clinical scope. |
| ARUP urine profile and detection-window table, DOM-120/121 | Listed metabolites and screen targets; detection windows are not used as decision rules or cutoffs. |
| FDA classification and product records, DOM-123/124 | Evidence of named test systems and specific device contexts. These records do not validate the starter method or establish general clinical prevalence. |
| Oral-fluid research methods, DOM-125–127 | Additional analyte/specimen examples. Research methods and populations are not clinical-panel recommendations. |

The menu excludes oral-fluid lorazepam, O-desmethyltramadol, noroxycodone, norhydrocodone, barbiturates, carisoprodol, gabapentin and ketamine; oral-fluid benzodiazepine/buprenorphine screens; and fentanyl analogues/xylazine. The 2026-10-13 review did not establish suitable source/terminology or clinical-method support for their inclusion. Zolpidem, methylphenidate/ritalinic acid and antidepressants are outside the scoped starter menu. These are scope/evidence limits, not claims that such testing cannot be performed.

## LOINC choice rule

The pinned release is **LOINC 2.83**, released in August 2026. LOINC 2.83 discourages drug terms whose method is "Screen" or "Confirm" and adds terms by immunoassay and mass spectrometry (DOM-119). Urine "Screen method" terms such as `19295-5` and `19261-7` are DISCOURAGED in 2.83.

For each analyte, the first **ACTIVE** term in this order wins. The specimen system must match exactly: "Urine", or "Saliva (oral fluid)".

| Rank | Screen analytes (presence) | Definitive analytes (mass concentration) |
| --- | --- | --- |
| 1 | `<component> [Presence] in <system> by Immunoassay` | `<component> [Mass/volume] in <system> by Mass Spectrometry` |
| 2 | `<component> [Presence] in <system>` (no method) | `<component> [Mass/volume] in <system>` (no method) |
| 3 (last resort) | `... by Screen method`, only while still ACTIVE | `... by Confirmatory method`, only while still ACTIVE |

Rules that apply to every rank:

- **Never used:** cutoff-specific variants (for example `Opiates [Presence] in Urine by Immunoassay>2000 ng/mL`), SAMHSA-method terms, and DISCOURAGED or DEPRECATED terms. The generic term is always preferred, because the cutoff belongs to the laboratory.
- **The component** is the assay's named target, for example "Opiates", "oxyCODONE" or "Cannabinoids". Combination components such as `oxyCODONE+oxyMORphone` were not used.
- **Validity analytes** use the method-less LOINC terms: `2161-8`, `2965-2`, `2756-5`, `97204-2` and `2657-5`.

Terminology evidence checked 2026-10-13 (DOM-128/129); no fresh service lookup is implied:

- **Search.** Candidates came from the NLM Clinical Tables search service (DOM-129), which did not report a release version and was not treated as the version authority.
- **Verification.** Each chosen code was looked up on the CSIRO Ontoserver public FHIR server (`CodeSystem/$lookup`, `version=2.83`, DOM-128). At that check, the service reported LOINC 2.83, ACTIVE status and a display equal to the long common name stored in the file. This is a third-party mirror; verification against the publisher remains advised.
- **Display name field.** `displayName` is the LOINC **Long Common Name**.

**Result:** every analyte has a code. 66 drug analytes use rank 1, 31 use rank 2 and 9 use rank 3. No analyte has `loinc: null`.

**Rank 3 picks.** These terms are ACTIVE in 2.83, but LOINC has said the screen/confirm family is being discouraged. Recheck them against each proposed terminology update and select an appropriate supported replacement when available; a terminology change still requires review of the local mapping:

- `OF-AMP-IA` (40799-9)
- `OF-COC-IA` (40802-1)
- `OF-6AM-IA` (87816-5)
- `OF-HYM` (72733-9)
- `OF-EDDP` (72385-8)
- `OF-BUP` (72762-8)
- `OF-TAP` (87819-9)
- `OF-AMP` (72758-6)
- `OF-COC` (72737-0)

## Verification

The offline cutoff pin is updated alongside the evidence table. Catalog tests check parsing and exact value/source transcription; engine tests check loading and demo behavior; report tests check current labels and continued reproduction of template v1. These are software checks on synthetic data, not clinical validation. Execute the applicable checks under the [quality-gate policy](../../../.ai/skills/quality-gate/SKILL.md); this content note does not attest to a test run.
