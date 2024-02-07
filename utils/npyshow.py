"""
Utility program to just show a .npy file outputted by main.rs for debugging
"""
import sys
import numpy as np
import pylab as pl

fname = sys.argv[1]
img = np.load(fname)
print(f"{img.shape=}, {img.dtype=}")
pl.imshow(img[..., :])
pl.colorbar()

pl.show()