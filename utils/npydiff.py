"""
Utility program to just show a .npy file outputted by main.rs for debugging
"""
import sys
import numpy as np
import pylab as pl

fname1 = sys.argv[1]
fname2 = sys.argv[2]

img1 = np.load(fname1)
img2 = np.load(fname2)
print(f"{img1.shape=}, {img2.dtype=}, {img2.shape=}, {img2.dtype=}")

pl.figure(figsize=(10,10))
pl.subplot(221)
pl.title(fname1)
pl.imshow(img1)

pl.subplot(222)
pl.title(fname2)
pl.imshow(img2)

pl.subplot(223)
pl.title(f'diff ({fname1} - {fname2})')
pl.imshow(img1.astype(np.float64) - img2.astype(np.float64))
pl.colorbar()

pl.show()