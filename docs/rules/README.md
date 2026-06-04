# Kural Kartları

Bu klasör, validator kurallarının kod yazarken ve davranış öğrenirken kullanılacak teknik kartlarını içerir.

Kartlar kural grubuna göre saklanır:

```text
docs/rules/<GRUP>/<RULE_ID>.md
```

Örnek:

```text
docs/rules/CAL/CAL_009.md
```

## Amaç

- Kuralın tam tetikleme mantığını koddan bağımsız okunabilir hale getirmek.
- Yanlış pozitif / yanlış negatif sınırlarını açık yazmak.
- Komşu kurallarla ayrımı netleştirmek.
- Notice alanlarını, skor etkisini ve minimum test matrisini belgelemek.
- Kod değiştirirken tüm projeyi taramadan doğru başlangıç noktasını vermek.
