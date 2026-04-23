// 控制台验证脚本：生成两个Ghost，展示人格差异和情感系统行为
// 运行方式: cargo run --bin verify_ghost

use deskpet_lib::core::ghost::Ghost;
use deskpet_lib::core::soul::emotional_event::{EmotionalEvent, EmotionalEventType};
use rand::thread_rng;

fn main() {
    println!("========================================");
    println!("  桌宠 Ghost 系统 - 控制台验证");
    println!("========================================\n");

    let mut rng = thread_rng();

    // 生成两个不同的Ghost
    let mut ghost_a = Ghost::generate(&mut rng, Some("小花".into()));
    let mut ghost_b = Ghost::generate(&mut rng, Some("阿铁".into()));

    // 展示人格差异
    println!("=== Ghost A: {} ===", ghost_a.name);
    print_personality(&ghost_a);

    println!("\n=== Ghost B: {} ===", ghost_b.name);
    print_personality(&ghost_b);

    // 展示初始情感状态
    println!("\n\n=== 初始情感状态 ===");
    print_emotional_state("小花", &ghost_a);
    print_emotional_state("阿铁", &ghost_b);

    // 模拟事件序列
    println!("\n\n=== 情感模拟 ===");
    let events = vec![
        (EmotionalEventType::FirstConversation, 1.0, "第一次对话"),
        (EmotionalEventType::UserCaredAboutPet, 0.8, "用户关心了桌宠"),
        (EmotionalEventType::NormalChat, 0.5, "普通聊天"),
        (EmotionalEventType::UserPraisedPet, 0.6, "用户夸奖了桌宠"),
        (EmotionalEventType::UserSharedPersonalStory, 0.7, "用户分享了个人经历"),
        (EmotionalEventType::UserGotAngry, 0.5, "用户生气了"),
        (EmotionalEventType::UserIgnoredPet, 0.8, "用户忽略了桌宠3天"),
        (EmotionalEventType::UserCaredAboutPet, 1.0, "用户再次表达了关心"),
        (EmotionalEventType::BirthdayCelebrated, 1.0, "一起庆祝了生日！"),
    ];

    for (i, (event_type, intensity, desc)) in events.iter().enumerate() {
        let event_a = EmotionalEvent::new_with_description(event_type.clone(), *intensity, desc.to_string());
        let event_b = EmotionalEvent::new_with_description(event_type.clone(), *intensity, desc.to_string());

        ghost_a.soul.apply_emotional_event(&event_a, &mut rng);
        ghost_b.soul.apply_emotional_event(&event_b, &mut rng);

        println!("\n--- 事件 {}: {} ---", i + 1, desc);
        print_emotional_state("小花", &ghost_a);
        print_emotional_state("阿铁", &ghost_b);
    }

    // 展示最终状态
    println!("\n\n=== 最终状态 ===");
    println!("小花: {}", ghost_a.soul.get_personality_description());
    println!("\n当前语气: {}", ghost_a.soul.sensibility.get_tone_instruction());
    println!("\n---");
    println!("阿铁: {}", ghost_b.soul.get_personality_description());
    println!("\n当前语气: {}", ghost_b.soul.sensibility.get_tone_instruction());

    // 模拟回弹
    println!("\n\n=== 理性回弹模拟 (100 ticks) ===");
    for i in 0..100 {
        ghost_a.soul.tick(&mut rng);
        ghost_b.soul.tick(&mut rng);
        if i % 20 == 0 {
            println!("Tick {}: 小花 LoveHate={:.1} (baseline={:.1}), 阿铁 LoveHate={:.1} (baseline={:.1})",
                i,
                ghost_a.soul.sensibility.love_hate,
                ghost_a.soul.sensibility.baseline,
                ghost_b.soul.sensibility.love_hate,
                ghost_b.soul.sensibility.baseline
            );
        }
    }

    println!("\n最终: 小花 LoveHate={:.1}, 阿铁 LoveHate={:.1}",
        ghost_a.soul.sensibility.love_hate,
        ghost_b.soul.sensibility.love_hate
    );

    println!("\n=== 验证完成 ===");
    println!("两个Ghost因为天生倾向不同，对相同的事件序列产生了不同的情感发展轨迹。");
    println!("这就是Ghost灵魂系统的核心价值：同一个世界观，不同的人格体验。");
}

fn print_personality(ghost: &Ghost) {
    let p = &ghost.soul.innate_tendency.self_dims;
    let pref = &ghost.soul.innate_tendency.preferred_dims;
    println!("  自我人格：");
    println!("    开放性={:.2} 尽责性={:.2} 外向性={:.2} 宜人性={:.2} 神经质={:.2} 创造性={:.2}",
        p.openness, p.conscientiousness, p.extraversion, p.agreeableness, p.neuroticism, p.creativity);
    println!("  喜好的主人：");
    println!("    开放性={:.2} 尽责性={:.2} 外向性={:.2} 宜人性={:.2} 神经质={:.2} 创造性={:.2}",
        pref.openness, pref.conscientiousness, pref.extraversion, pref.agreeableness, pref.neuroticism, pref.creativity);
    println!("  移动风格: {}", ghost.body.movement_style);
    println!("  理性回弹系数: {:.4}", ghost.soul.sensibility.base_damping);
}

fn print_emotional_state(name: &str, ghost: &Ghost) {
    let s = &ghost.soul.sensibility;
    let aff = &ghost.soul.impression.overall_affinity;
    let tone = s.get_tone_instruction();
    println!("  {}: LoveHate={:+.1} baseline={:.1} 综合好感度={:.1} 语气=[{}]",
        name, s.love_hate, s.baseline, aff, tone.chars().take(15).collect::<String>());
}