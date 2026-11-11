#table(
  columns: 2,
  [PlainCellAlpha], [PlainCellBeta],
  [MultiCellLineOne\ MultiCellLineTwo], "StringCellGamma",
  ..range(2).map(i => [GeneratedCell #i]),
)
