#!/bin/bash


set -ex


ALL_PREI=(RCC USART1 I2C1 SPI1 EXTI GPIOA AFIO OPA TIM1 CAN1 ADC1 DAC USBPD FLASH FSMC WWDG)


for peri in ${ALL_PREI[@]}; do
    ./d extract-all $peri
done


