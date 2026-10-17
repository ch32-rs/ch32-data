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
            // CH32H4 routes every request through one DMAMUX, so its include is
            // keyed `DMAMUX` and the numbers in the yaml are request IDs, not
            // channel numbers. The channel list is fixed (DMA1 and DMA2, 8 each)
            // and every signal is reachable from every channel.
            let mut mux_requests: Vec<(String, u8)> = Vec::new();

            for (dma, inc_path) in dma_channels_inc {
                let dma_yaml_path = meta_yaml_path.parent().unwrap().join(&inc_path);
                let content = std::fs::read_to_string(&dma_yaml_path)?;
                let dma_map: HashMap<String, u8> = serde_yaml::from_str(&content)?;

                if dma == "DMAMUX" {
                    for (signal, &request) in &dma_map {
                        if mux_requests.iter().any(|(s, _)| s == signal) {
                            anyhow::bail!(
                                "DMAMUX request for {} already exists in core {}",
                                signal,
                                core.name
                            );
                        }
                        mux_requests.push((signal.clone(), request));
                    }
                    continue;
                }

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

            if !mux_requests.is_empty() {
                for dma in ["DMA1", "DMA2"] {
                    for i in 0..8 {
                        core.dma_channels
                            .push(ch32_data_serde::chip::core::DmaChannels {
                                name: format!("{}_CH{}", dma, i + 1),
                                dma: dma.to_string(),
                                channel: i, // 0 based
                            });
                    }
                }

                for (signal, request) in &mux_requests {
                    let (periph, signal_name) = split_signal(signal);
                    for p in &mut core.peripherals {
                        if p.name == periph {
                            p.dma_channels.push(
                                ch32_data_serde::chip::core::peripheral::DmaChannel {
                                    signal: signal_name.to_string(),
                                    // Any of the 16 channels can carry it; the
                                    // request number is what selects the source.
                                    channel: None,
                                    dma: None,
                                    dmamux: Some("DMAMUX".to_string()),
                                    request: Some(*request),
                                },
                            )
                        }
                    }
                }
            }

            // channel is 1-based number
            for (dma, signal, channel) in &all_dma_signals {
                let (periph, signal_name) = split_signal(signal);
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

/// Split `USART1_TX` into (`USART1`, `TX`). A key with no `_` (`SDIO`, `ADC1`)
/// is both the peripheral and the signal, matching the fixed-channel tables.
fn split_signal(signal: &str) -> (&str, &str) {
    match signal.split_once('_') {
        Some((periph, signal_name)) => (periph, signal_name),
        None => (signal, signal),
    }
}
