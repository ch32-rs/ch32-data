use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

pub fn handle_chip_dma_include<P: AsRef<Path>>(
    path: P,
    chip: &mut ch32_data_serde::Chip,
) -> anyhow::Result<()> {
    let meta_yaml_path = path.as_ref();

    for core in &mut chip.cores {
        // DMA, Signal, Channel
        // e.g. DMA1, ADC1, 1
        let mut all_dma_signals: BTreeSet<(String, String, u8)> = BTreeSet::new();

        if let Some(dma_channels_inc) = core.include_dma_channels.take() {
            for (dma, inc_path) in dma_channels_inc {
                let dma_yaml_path = meta_yaml_path.parent().unwrap().join(&inc_path);
                let content = std::fs::read_to_string(&dma_yaml_path)?;
                let dma_map: HashMap<String, u8> = serde_yaml::from_str(&content)?;

                for (signal, &channel) in &dma_map {
                    let dma_signal = (dma.clone(), signal.clone(), channel);
                    if all_dma_signals.contains(&dma_signal) {
                        anyhow::bail!("DMA signal {} already exists in core {}", dma, core.name);
                    }
                    all_dma_signals.insert(dma_signal);
                }

                let mut dma_map: Vec<(String, u8)> = dma_map.into_iter().collect();
                dma_map.sort_by_key(|(_, number)| *number);

                let max_ch = dma_map.iter().map(|(_, channel)| *channel).max().unwrap();

                /* Insert Chip core level dma_channels
                cores[0].dma_channels
                {
                    "name": "DMA1_CH1",
                    "dma": "DMA1",
                    "channel": 0
                },
                */
                for i in 0..max_ch {
                    let name = format!("{}_CH{}", dma, i + 1);
                    let channel = i; // 0 based
                    core.dma_channels
                        .push(ch32_data_serde::chip::core::DmaChannels {
                            name,
                            dma: dma.clone(),
                            channel,
                        });
                }
            }

            // channel is 1-based number
            for (dma, signal, channel) in &all_dma_signals {
                let periph = signal.split('_').next().unwrap();

                let signal_name = if signal.contains('_') {
                    signal.split('_').skip(1).next().unwrap()
                } else {
                    signal
                };
                // "signal": "M",
                // "channel": "DMA1_CH2"
                let channel_name = format!("{}_CH{}", dma, channel);

                for p in &mut core.peripherals {
                    if p.name == periph {
                        //println!(
                        //    "found DMA singal {} {} {}",
                        //    periph, signal_name, channel_name
                        //);
                        p.dma_channels
                            .push(ch32_data_serde::chip::core::peripheral::DmaChannel {
                                signal: signal_name.to_string(),
                                channel: Some(channel_name.to_string()),
                                dma: None, // TODO: rm these unused fields, as they only exist in STM32
                                dmamux: None,
                                request: None,
                            })
                    }
                }
            }
        }
    }

    Ok(())
}
