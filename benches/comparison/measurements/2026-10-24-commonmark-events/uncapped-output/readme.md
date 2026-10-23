# Superseded full-source reservation

This complete evaluation used source `257bf14d21e7738cea8463b1dc8576630da9ce14`, whose patch against merged main is retained in `source.patch`. Its HTML buffer reserved the entire input length. All samples and correctness checks are retained, but these are not the final PR measurements.

The initial scanner observation showed slower long prose and fenced code. Three alternating `scanner-check` rounds isolated baseline, prefix/span changes without reservation (`spans`), and full-source reservation (`candidate`) at one launch path. The long-prose regression repeated; the code timings were variable. Full-source reservation was replaced with a 4 KiB initial cap. The final root-level evaluation repeats scanner workloads in all three alternating comparison rounds.
