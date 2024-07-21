local utils = require('utils')
local symmetries = require('symmetries')

puzzles:add('fto', {
  ndim = 3,
  name = "Face-Turning Octahedron",
  colors = 'octahedron',
  build = function(self)
    local sym = cd'bc3'
    local shape = symmetries.octahedral.octahedron()
    self:carve(shape:iter_poles())

    -- Define axes and slices
    self.axes:add(shape:iter_poles(), utils.layers_exclusive(1, -1, 3))

    -- Define twists
    for _, axis, twist_transform in sym.chiral:orbit(self.axes[sym.xoo.unit], sym:thru(2, 3)) do
      self.twists:add(axis, twist_transform)
    end
  end,
})