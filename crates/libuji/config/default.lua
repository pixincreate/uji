-- Default uji UI.
-- Message history fills the top (Pi-style blocks); input box at the bottom.

uji.open_win("messages", {
  split = "top",
  size = "fill",
  border = "none",
})

uji.open_win("input", {
  split = "bottom",
  size = 3,
  border = "plain",
})
