file esp/KERNEL.ELF
b ysos_kernel::init
python
try:
    gdb.execute("target remote localhost:1234")
except gdb.error:
    pass
end
