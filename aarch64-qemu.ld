__virt_start = 0xFFFF000000000000;
ENTRY(kernel_init)

SECTIONS
{
    /* Set current address to the value from which the RPi starts execution */
    . = __virt_start;
    __ro_start = .;

    .text :
    {
        *(.text*)
    
    .exception_vectors :
    {
        *(.exception_vectors*)
    }

    .rodata ALIGN(4K) :
    {
        *(.rodata*)
    }
    . = ALIGN(4K); 
    __ro_end = .;

    .data :
    {
        *(.data*)
    }

    /* Section is zeroed in u64 chunks, align start and end to 8 bytes */
    .bss ALIGN(8): 
    {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(8);

        /* Fill for the bss == 0 case, so that __bss_start <= __bss_end_inclusive holds */
        . += 8;
        __bss_end_inclusive = . - 8;
    }
    . = ALIGN(4K);
    __data_end = .;
    
    . = ALIGN(8);
    . = . + 0x4000;
    LD_STACK_PTR = .;

    __ro_size = __ro_end - __ro_start;
    __data_size = __data_end - __ro_end;

    /DISCARD/ : { *(.comment*) }
}
